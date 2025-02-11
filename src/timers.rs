#![doc = include_str!("doc_timers.md")]

use std::{
	convert::TryFrom,
	fmt,
	sync::mpsc::Receiver,
	thread,
	time::{Duration, Instant},
};

use midly::Timing;

use crate::{Event, Moment, Timer};

/// An error that might arise while converting [Timing] to a [Ticker] or
/// [FixedTempo].
pub struct TimeFormatError;

impl std::error::Error for TimeFormatError {}

impl fmt::Debug for TimeFormatError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.write_str("unsupported time format")
	}
}

impl fmt::Display for TimeFormatError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.write_str("unsupported time format")
	}
}

/// Implements a Metrical [Timer].
///
/// Use this when the MIDI file header specifies the time format as being
/// [Timing::Metrical], this is the case 99% of the time.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Ticker {
	ticks_per_beat: u16,
	micros_per_tick: f64,
	last_instant: Option<Instant>,
	/// Speed modifier, a value of `1.0` is the default and affects nothing.
	///
	/// Important: Do not set to 0.0, this value is used as a denominator.
	pub speed: f32,
}

impl Ticker {
	/// Create an instance of a [Ticker] with the given ticks-per-beat.
	///
	/// The tempo will be infinitely rapid, meaning no sleeps will happen.
	/// However this is rarely an issue since a tempo change message will set
	/// it, and this usually happens before any non-0 offset event.
	pub const fn new(ticks_per_beat: u16) -> Self {
		Self {
			ticks_per_beat,
			micros_per_tick: 0.0,
			last_instant: None,
			speed: 1.0,
		}
	}

	/// Create an instance of a [Ticker] with a provided tempo.
	pub fn with_initial_tempo(ticks_per_beat: u16, tempo: u32) -> Self {
		let mut s = Self::new(ticks_per_beat);
		s.change_tempo(tempo);
		s
	}

	/// Upgrades `self` to a [ControlTicker].
	pub fn to_control(self, pause: Receiver<()>) -> ControlTicker {
		ControlTicker {
			speed: self.speed,
			micros_per_tick: self.micros_per_tick,
			last_instant: self.last_instant,
			ticks_per_beat: self.ticks_per_beat,
			pause,
		}
	}

	/// Calculate the duration of `n_ticks` ticks, without accounting for the last time this [Ticker] ticked.
	/// This is useful for calculating the duration of a song, for example.
	pub fn sleep_duration_without_readjustment(&self, n_ticks: u32) -> Duration {
		let t = self.micros_per_tick * n_ticks as f64 / self.speed as f64;

		if t > 0.0 {
			Duration::from_micros(t as u64)
		} else {
			Duration::default()
		}
	}
}

impl Timer for Ticker {
	fn change_tempo(&mut self, tempo: u32) {
		let micros_per_tick = tempo as f64 / self.ticks_per_beat as f64;
		self.micros_per_tick = micros_per_tick;
	}

	fn sleep_duration(&mut self, n_ticks: u32) -> Duration {
		let mut t = self.sleep_duration_without_readjustment(n_ticks);

		match self.last_instant {
			Some(last_instant) => {
				self.last_instant = Some(last_instant + t);
				t = t.checked_sub(last_instant.elapsed()).unwrap_or(t);
			}
			None => self.last_instant = Some(Instant::now()),
		}

		t
	}

	fn duration(&mut self, moments: &[Moment]) -> Duration {
		let mut counter = Duration::default();

		for moment in moments {
			counter += self.sleep_duration_without_readjustment(1);

			for event in &moment.events {
				if let Event::Tempo(val) = event {
					self.change_tempo(*val);
				}
			}
		}

		counter
	}
}

impl TryFrom<Timing> for Ticker {
	type Error = TimeFormatError;

	/// Tries to create a [Ticker] from the provided [Timing].
	///
	/// # Errors
	/// Will return an error if the given [Timing] is not [Timing::Metrical].
	fn try_from(t: Timing) -> Result<Self, Self::Error> {
		match t {
			Timing::Metrical(n) => Ok(Self::new(u16::from(n))),
			_ => Err(TimeFormatError),
		}
	}
}

/// A [Timer] with a fixed tempo.
///
/// The value wrapped corresponds to the length of a tick, in microseconds.
///
/// # Notes
/// This type corresponds to [Timing::Timecode] and can be converted using
/// [TryFrom::try_from]. Try to avoid using this timer because it's not tested
/// (I couldn't find any MIDI files using [Timing::Timecode]).
pub struct FixedTempo(pub u64);

impl TryFrom<Timing> for FixedTempo {
	type Error = TimeFormatError;

	fn try_from(t: Timing) -> Result<Self, Self::Error> {
		if let Timing::Timecode(fps, frame) = t {
			let micros = 1_000_000.0 / fps.as_f32() / frame as f32;
			Ok(Self(micros as u64))
		} else {
			Err(TimeFormatError)
		}
	}
}

impl Timer for FixedTempo {
	fn sleep_duration(&mut self, n_ticks: u32) -> Duration {
		Duration::from_millis(self.0 * n_ticks as u64)
	}

	/// This function does nothing.
	fn change_tempo(&mut self, _: u32) {}
}

/// A [Timer] that lets you toggle playback.
///
/// This type works exactly like [Ticker], but it checks for messages
/// on a [Receiver] and toggles playback if there is one.
///
/// Sending a message to [self.pause] will pause the thread until another
/// message is received.
///
/// # Notes
/// Using [Ticker] is recommended over this, mainly because there is the
/// overhead of [Receiver] with this type.
///
/// Calling [sleep](Self::sleep) will panic if the corresponding end of the
/// receiver is poisoned, see the [mpsc](std::sync::mpsc) documentation for
/// more.
#[derive(Debug)]
pub struct ControlTicker {
	ticks_per_beat: u16,
	micros_per_tick: f64,
	last_instant: Option<Instant>,
	/// Speed modifier, a value of `1.0` is the default and affects nothing.
	///
	/// Important: Do not set to 0.0, this value is used as a denominator.
	pub speed: f32,
	/// Messages to this channel will toggle playback.
	pub pause: Receiver<()>,
}

impl ControlTicker {
	/// Create an instance of [ControlTicker] with the given ticks-per-beat.
	/// The tempo will be infinitely rapid, meaning no sleeps will happen.
	/// However this is rarely an issue since a tempo change message will set
	/// it, and this usually happens before any non-0 offset event.
	pub fn new(ticks_per_beat: u16, pause: Receiver<()>) -> Self {
		Self {
			ticks_per_beat,
			pause,
			last_instant: None,
			micros_per_tick: 0.0,
			speed: 1.0,
		}
	}

	/// Create an instance of [ControlTicker] with a provided tempo.
	pub fn with_initial_tempo(ticks_per_beat: u16, tempo: u32, pause: Receiver<()>) -> Self {
		let mut s = Self::new(ticks_per_beat, pause);
		s.change_tempo(tempo);
		s
	}

	/// Get a [Ticker].
	pub fn to_ticker(&self) -> Ticker {
		Ticker {
			ticks_per_beat: self.ticks_per_beat,
			micros_per_tick: self.micros_per_tick,
			last_instant: None,
			speed: self.speed,
		}
	}

	/// Calculate the duration of `n_ticks` ticks, without accounting for the last time this [Ticker] ticked.
	/// This is useful for calculating the duration of a song, for example.
	pub fn sleep_duration_without_readjustment(&self, n_ticks: u32) -> Duration {
		let t = self.micros_per_tick * n_ticks as f64 / self.speed as f64;

		if t > 0.0 {
			Duration::from_micros(t as u64)
		} else {
			Duration::default()
		}
	}
}

impl Timer for ControlTicker {
	fn change_tempo(&mut self, tempo: u32) {
		let micros_per_tick = tempo as f64 / self.ticks_per_beat as f64;
		self.micros_per_tick = micros_per_tick;
	}

	fn sleep_duration(&mut self, n_ticks: u32) -> Duration {
		let mut t = self.sleep_duration_without_readjustment(n_ticks);

		match self.last_instant {
			Some(last_instant) => {
				self.last_instant = Some(last_instant + t);
				t = t.checked_sub(last_instant.elapsed()).unwrap_or(t);
			}
			None => self.last_instant = Some(Instant::now()),
		}

		t
	}

	/// Same with [Ticker::sleep], except it checks if there are any messages on
	/// [self.pause], if there is a message, waits for another one before
	/// continuing with the sleep.
	fn sleep(&mut self, n_ticks: u32) {
		// Check if we're supposed to be paused.
		if self.pause.try_recv().is_ok() {
			// Wait for the next message in order to continue, continue.
			self.pause
				.recv()
				.unwrap_or_else(|e| panic!("ControlTicker: pause channel receive failed: {:?}", e));

			self.last_instant = None;
		}

		let t = self.sleep_duration(n_ticks);

		if !t.is_zero() {
			sleep(t);
		}
	}

	fn duration(&mut self, moments: &[Moment]) -> Duration {
		let mut counter = Duration::default();

		for moment in moments {
			counter += self.sleep_duration_without_readjustment(1);

			for event in &moment.events {
				if let Event::Tempo(val) = event {
					self.change_tempo(*val);
				}
			}
		}

		counter
	}
}

/// Pauses the thread for the provided duration.
///
/// Sleeps with [thread::sleep] for most of the time
/// and spin-locks for the last T milliseconds, where T:
/// - Windows: 15.
/// - Non-Windows: 3.
#[cfg(any(doc, test, feature = "hybrid-sleep"))]
pub fn sleep(t: Duration) {
	use std::time::Instant;
	#[cfg(windows)]
	const LIMIT: Duration = Duration::from_millis(15);
	#[cfg(not(windows))]
	const LIMIT: Duration = Duration::from_millis(3);

	let t = if t < LIMIT {
		t
	} else {
		let mut last = Instant::now();
		let mut remaining = t;
		loop {
			thread::sleep(Duration::from_millis(1));
			let now = Instant::now();
			remaining = remaining.checked_sub(now - last).unwrap_or_default();
			if remaining <= LIMIT {
				break remaining;
			}
			last = now;
		}
	};
	spin_lock(t);
}

#[cfg(feature = "hybrid-sleep")]
#[inline]
fn spin_lock(t: Duration) {
	let now = std::time::Instant::now();
	while now.elapsed() < t {
		std::hint::spin_loop();
	}
}

#[cfg(not(any(doc, test, feature = "hybrid-sleep")))]
pub(crate) use thread::sleep;
