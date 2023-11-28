use core::{
    cell::RefCell,
    task::{Poll, Waker},
};

use critical_section::Mutex;
use fugit::MicrosDurationU32;
use futures::{future, Future};
use panic_probe as _;

use super::hal::{
    pac::interrupt,
    timer::{Alarm, Alarm0, },
};
pub use super::hal::timer::Timer;

type Alarm0WakerCTX = (Alarm0, Option<Waker>);
pub static ALARM0_WAKER: Mutex<RefCell<Option<Alarm0WakerCTX>>> = Mutex::new(RefCell::new(None));
pub async fn wait_for(timer: &Timer, delay: u32) {
    if delay < 20 {
        let start = timer.get_counter_low();
        future::poll_fn(|cx| {
            if timer.get_counter_low().wrapping_sub(start) < delay {
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    } else {
        let mut started = false;
        future::poll_fn(move |cx| {
            critical_section::with(|cs| {
                if let Some((alarm, waker)) = ALARM0_WAKER.borrow_ref_mut(cs).as_mut() {
                    if !started {
                        alarm.clear_interrupt();
                        alarm.enable_interrupt();
                        alarm.schedule(MicrosDurationU32::micros(delay)).unwrap();
                        started = true;
                        *waker = Some(cx.waker().clone());
                        Poll::Pending
                    } else if alarm.finished() {
                        Poll::Ready(())
                    } else {
                        *waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                } else {
                    unreachable!()
                }
            })
        })
        .await;
    }
}

pub async fn timed<T>(op: &str, timer: &Timer, fut: impl Future<Output = T>) -> T {
    let start = timer.get_counter_low();
    let res = fut.await;
    let diff = timer.get_counter_low().wrapping_sub(start);
    defmt::info!("{} took {}us", op, diff);
    res
}

#[interrupt]
#[allow(non_snake_case)]
fn TIMER_IRQ_0() {
    critical_section::with(|cs| {
        let mut binding = ALARM0_WAKER.borrow_ref_mut(cs);
        let Some((alarm, waker)) = binding.as_mut() else {
            unreachable!()
        };
        alarm.disable_interrupt();
        waker.take().map(Waker::wake);
    });
}
