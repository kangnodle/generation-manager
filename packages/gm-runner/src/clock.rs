use gm_application::Clock;
use std::time::SystemTime;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime { SystemTime::now() }
}
