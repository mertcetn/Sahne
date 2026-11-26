use super::Result;
use std::{fs, io::Write, sync::mpsc, thread, time::Duration};
use wallpaper_protocol::{Body, Message, read_frame, write_frame};

pub fn check(folder: &str) -> Result<()> {
    let snapshot = wallpaper_platform::package_directory::collect_snapshot(folder)?;
    let exe = std::env::current_exe()?.with_file_name("wallpaper-sandbox-probe.exe");
    let session = wallpaper_sandbox::random_id()?;
    let nonce = wallpaper_sandbox::random_id()?;
    let base = std::env::current_dir()?.join("target/sandbox-runs");
    fs::create_dir_all(&base)?;
    let root = base.join(format!("intake-{}-{session:016x}", std::process::id()));
    let wallpaper_sandbox::Channel {
        mut sandbox,
        mut input,
        mut output,
    } = wallpaper_sandbox::spawn_probe(&exe, &root, session, "package-snapshot", 0, None)?;

    let (sender, receiver) = mpsc::sync_channel(1);
    let channel = thread::Builder::new()
        .name("package-intake-channel".into())
        .spawn(move || {
            let result = (|| -> Result<bool> {
                let hello = read_frame(&mut output)?.ok_or("Missing package helper Hello")?;
                hello.check_context(session, 1, 0)?;
                if hello.body != (Body::Hello { api_version: 1 }) {
                    return Err("Unexpected package helper Hello".into());
                }
                write_frame(
                    &mut input,
                    &Message {
                        session,
                        sequence: 1,
                        base_revision: 0,
                        body: Body::Ping { nonce },
                    },
                )?;
                snapshot.write_to(&mut input)?;
                input.flush()?;
                drop(input);
                let reply = read_frame(&mut output)?.ok_or("Missing package helper reply")?;
                reply.check_context(session, 2, 0)?;
                let accepted = match reply.body {
                    Body::Pong { nonce: echoed } if echoed == nonce => true,
                    Body::Reject { code: 1, message } if message == "Package validation failed" => {
                        false
                    }
                    _ => return Err("Unexpected package helper reply".into()),
                };
                if read_frame(&mut output)?.is_some() {
                    return Err("Extra package helper reply".into());
                }
                Ok(accepted)
            })()
            .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
    let channel = match channel {
        Ok(channel) => channel,
        Err(error) => {
            sandbox.cleanup_profile()?;
            return Err(error.into());
        }
    };
    let reply = receiver.recv_timeout(Duration::from_secs(8));
    let exit = if matches!(reply, Ok(Ok(_))) {
        sandbox.wait(1000)
    } else {
        Ok(None)
    };
    let stopped = sandbox.stop();
    if let Err(error) = stopped {
        drop(sandbox);
        let _ = channel.join();
        return Err(error.into());
    }
    let joined = channel.join();
    sandbox.cleanup_profile()?;
    joined.map_err(|_| "Package intake channel panicked")?;
    println!("Package helper profile cleanup verified.");
    let accepted = reply??;
    if exit? != Some(0) {
        return Err("Package helper did not exit cleanly".into());
    }
    if !accepted {
        return Err("Package rejected by AppContainer validation".into());
    }
    println!("Package validated in AppContainer.");
    Ok(())
}
