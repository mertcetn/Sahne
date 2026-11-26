use super::Result;
use std::{
    fs,
    io::{self, Read, Write},
    sync::mpsc,
    thread,
    time::Duration,
};
use wallpaper_media::flat_output::{self, FlatOutput};
use wallpaper_protocol::{Body, Message, read_frame, write_frame};

enum IntakeReply {
    Compiled(FlatOutput, Option<Body>),
    Rejected(&'static str),
}

pub fn check(folder: &str) -> Result<()> {
    let compiled = load(folder)?;
    println!("Package validated and compiled in AppContainer.");
    println!(
        "Host independently validated flat output. Nodes: {}. Images: {}.",
        compiled.scene.scene().nodes.len(),
        compiled.images.len()
    );
    Ok(())
}

pub fn load(folder: &str) -> Result<FlatOutput> {
    load_package(folder, false).map(|(output, _)| output)
}

pub fn load_script(folder: &str) -> Result<(FlatOutput, Body)> {
    let (output, program) = load_package(folder, true)?;
    Ok((output, program.ok_or("Missing isolated script program")?))
}

fn load_package(folder: &str, scripted: bool) -> Result<(FlatOutput, Option<Body>)> {
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
    } = wallpaper_sandbox::spawn_probe(
        &exe,
        &root,
        session,
        if scripted {
            "package-script-snapshot"
        } else {
            "package-snapshot"
        },
        0,
        None,
    )?;

    let (sender, receiver) = mpsc::sync_channel(1);
    let channel = thread::Builder::new()
        .name("package-intake-channel".into())
        .spawn(move || {
            let result = (|| -> Result<IntakeReply> {
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
                let result = match reply.body {
                    Body::Pong { nonce: echoed } if echoed == nonce => {
                        let scene = flat_output::read(&mut output)?;
                        let program = if scripted {
                            let message = read_frame(&mut output)?
                                .ok_or("Missing isolated script program")?;
                            message.check_context(session, 3, 0)?;
                            Some(message.body)
                        } else {
                            None
                        };
                        IntakeReply::Compiled(scene, program)
                    }
                    Body::Reject { code: 1, message } if message == "Package validation failed" => {
                        IntakeReply::Rejected("Package rejected by AppContainer validation")
                    }
                    Body::Reject { code: 2, message }
                        if message == "Package compilation failed" =>
                    {
                        IntakeReply::Rejected("Package rejected by AppContainer compilation")
                    }
                    Body::Reject { code: 3, message }
                        if scripted && message == "Package script preparation failed" =>
                    {
                        IntakeReply::Rejected("Package rejected by AppContainer script preparation")
                    }
                    _ => return Err("Unexpected package helper reply".into()),
                };
                match output.read_exact(&mut [0]) {
                    Ok(()) => return Err("Extra package helper data".into()),
                    Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {}
                    Err(error) => return Err(error.into()),
                }
                Ok(result)
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
    let result = reply??;
    if exit? != Some(0) {
        return Err("Package helper did not exit cleanly".into());
    }
    match result {
        IntakeReply::Compiled(compiled, program) => Ok((compiled, program)),
        IntakeReply::Rejected(reason) => Err(reason.into()),
    }
}
