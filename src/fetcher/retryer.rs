use rand::{thread_rng, Rng};
use std::{cmp::min, thread, time::Duration};

use crate::Error;

const MIN_RETRY_DELAY_MILLIS: u32 = 100;
const MAX_RETRY_DELAY_MILLIS: u32 = 15000;
const BASE_RETRY_SCALING_MILLIS: u32 = 50;

pub(crate) struct Retryer {
	delay_millis: u32,
	delay_scaling_millis: u32,
}

impl Retryer {
	pub(crate) fn new() -> Self {
		Retryer {
			delay_millis: MIN_RETRY_DELAY_MILLIS,
			delay_scaling_millis: BASE_RETRY_SCALING_MILLIS,
		}
	}

	pub(crate) fn reset(&mut self) {
		self.delay_millis = MIN_RETRY_DELAY_MILLIS;
		self.delay_scaling_millis = BASE_RETRY_SCALING_MILLIS;
	}

	#[allow(clippy::result_large_err)] // The error variant should never happen, so it's OK
	pub(crate) fn failure(&mut self) -> Result<(), Error> {
		let snooze_time_millis = self
			.delay_millis
			.checked_add(
				thread_rng()
					.gen::<u32>()
					.rem_euclid(self.delay_scaling_millis),
			)
			.ok_or_else(|| Error::arithmetic("calculating snooze_time_millis"))?;
		thread::sleep(Duration::from_millis(snooze_time_millis.into()));
		self.delay_millis = min(
			self.delay_millis
				.checked_mul(2)
				.ok_or_else(|| Error::arithmetic("doubling delay_millis"))?,
			MAX_RETRY_DELAY_MILLIS,
		);
		self.delay_scaling_millis = self
			.delay_scaling_millis
			.checked_add(BASE_RETRY_SCALING_MILLIS)
			.ok_or_else(|| Error::arithmetic("increasing delay_scaling_millis"))?;

		Ok(())
	}
}
