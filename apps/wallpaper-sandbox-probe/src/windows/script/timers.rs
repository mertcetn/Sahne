//! Fixed probe of timer registration and cancellation in an isolated JS context.
use super::Result;
use rquickjs::{Context, Function};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};
use wallpaper_scene::timer::{MIN_DELAY, TimerId, Timers};

pub(super) fn check(context: &Context) -> Result<()> {
    let start = Instant::now();
    let timers = Rc::new(RefCell::new(Timers::new(start)));
    let handles = Rc::new(RefCell::new(BTreeMap::<u32, TimerId>::new()));
    context.with(|ctx| {
        let schedule_timers = Rc::clone(&timers);
        let schedule_handles = Rc::clone(&handles);
        let next = Cell::new(1u32);
        let schedule = Function::new(ctx.clone(), move |delay: u32, repeating: bool| {
            let handle = next.get();
            let Some(following) = handle.checked_add(1) else {
                return 0;
            };
            let Ok(id) = schedule_timers.borrow_mut().schedule(
                start,
                Duration::from_millis(u64::from(delay)),
                repeating,
            ) else {
                return 0;
            };
            next.set(following);
            schedule_handles.borrow_mut().insert(handle, id);
            handle
        })?;
        let cancel_timers = Rc::clone(&timers);
        let cancel_handles = Rc::clone(&handles);
        let cancel = Function::new(ctx.clone(), move |handle: u32| {
            cancel_handles
                .borrow_mut()
                .remove(&handle)
                .is_some_and(|id| cancel_timers.borrow_mut().cancel(id))
        })?;
        let setup: Function = ctx.eval(include_str!("timer_fixture.js"))?;
        let dispatch: Function = setup.call((schedule, cancel))?;
        let one_shot = *handles.borrow().get(&1).ok_or("Missing timeout handle")?;
        let interval = *handles.borrow().get(&3).ok_or("Missing interval handle")?;
        let take_due = |at| {
            timers
                .borrow_mut()
                .take_due(at)
                .map_err(|_| "Timer dispatch failed")
        };
        if take_due(start)?.is_some() || take_due(start + MIN_DELAY)? != Some(one_shot) {
            return Err("Unexpected one-shot dispatch".into());
        }
        handles.borrow_mut().remove(&1);
        if dispatch.call::<_, u32>((1,))? != 1 {
            return Err("One-shot callback did not run".into());
        }
        timers
            .borrow_mut()
            .set_paused(start + MIN_DELAY, true)
            .map_err(|_| "Could not pause fixture timers")?;
        let resumed = start + Duration::from_secs(1);
        if take_due(resumed)?.is_some() {
            return Err("Paused interval dispatched".into());
        }
        timers
            .borrow_mut()
            .set_paused(resumed, false)
            .map_err(|_| "Could not resume fixture timers")?;
        if take_due(resumed)? != Some(interval)
            || take_due(resumed)?.is_some()
            || dispatch.call::<_, u32>((3,))? != 11
            || timers.borrow().next_wake().is_some()
        {
            return Err("Unexpected resumed interval or cancellation".into());
        }
        Ok(())
    })
}
