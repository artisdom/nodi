use ws2818_rgb_led_spi_driver::adapter_gen::WS28xxAdapter;
use ws2818_rgb_led_spi_driver::adapter_spi::WS28xxSpiAdapter;

#[cfg(feature = "midir")]
use midir::{self, MidiOutputConnection};
use midly::{
	live::{SystemCommon, SystemRealtime},
	MidiMessage,
};

use crate::{
	event::{Event, MidiEvent, Moment},
	Timer,
};

#[doc = include_str!("doc_player.md")]
pub struct Player<T: Timer, C: Connection> {
	/// An active midi connection.
	pub con: C,
	timer: T,
}

impl<T: Timer, C: Connection> Player<T, C> {
	/// Creates a new [Player] with the given [Timer] and
	/// [Connection].
	pub fn new(timer: T, con: C) -> Self {
		Self { con, timer }
	}

	/// Changes `self.timer`, returning the old one.
	pub fn set_timer(&mut self, timer: T) -> T {
		std::mem::replace(&mut self.timer, timer)
	}

	/// Plays the given [Moment] slice.
	///
	/// # Notes
	/// The tempo change events are handled by `self.timer` and playing sound by
	/// `self.con`.
	///
	/// Stops playing if [Connection::play] returns `false`.
	/// Returns `true` if the track is played through the end, `false` otherwise.
	pub fn play(&mut self, sheet: &[Moment]) -> bool {
		let mut counter = 0_u32;
		let mut adapter = WS28xxSpiAdapter::new("/dev/spidev0.0").unwrap();
		let (num_leds, r, g, b) = (176, 0, 0, 0);
		let mut data = vec![(r, g, b); num_leds];
		adapter.write_rgb(&data).unwrap();

		for moment in sheet {
			if !moment.is_empty() {
				self.timer.sleep(counter);
				counter = 0;

				for event in &moment.events {
					match event {
						Event::Tempo(val) => self.timer.change_tempo(*val),
						Event::Midi(msg) => {
							match msg.message {
								MidiMessage::NoteOn { key, vel } => {
									data[key.as_int() as usize * 2] = (0, 0, ((vel.as_int() as f32 / 127.0) * (100.0 / 10.0)) as u8);
									// data[key.as_int() as usize * 2] = (0, 0, u8::from(vel));
									// data[key.as_int() as usize * 2] = (0, 0, 100);
									// data[key.as_int() as usize * 2] = (0, 0, 10);
									adapter.write_rgb(&data).unwrap();
								}
								MidiMessage::NoteOff { key, vel: _ } => {
									data[key.as_int() as usize * 2] = (0, 0, 0);
									adapter.write_rgb(&data).unwrap();
								}
								_ => (),
							}

							if !self.con.play(*msg) {
								return false;
							}
						}
						_ => (),
					};
				}
			}

			counter += 1;
		}

		let data_clear = vec![(0, 0, 0); num_leds];
		adapter.write_rgb(&data_clear).unwrap();

		true
	}
}

/// Any type that can play sound, given a [MidiEvent].
///
/// This trait is implemented for midir::MidiOutputConnection, if the `midir`
/// feature is enabled.
pub trait Connection {
	/// Given a [MidiEvent], plays the message.
	///
	/// If this function returns `false`, [Player::play] will stop playing and return.
	fn play(&mut self, event: MidiEvent) -> bool;

	/// Sends a system realtime message.
	///
	/// The default implementation of this method does nothing.
	fn send_sys_rt(&mut self, _msg: SystemRealtime) {}

	/// Sends a system common message.
	///
	/// The default implementation of this method does nothing.
	fn send_sys_common(&mut self, _msg: SystemCommon<'_>) {}

	/// Turns all notes off.
	///
	/// The provided implementation simply blasts every channel with NoteOff messages for every possible note; `16 * 128 = 2048` messages will be sent.
	fn all_notes_off(&mut self) {
		for ch in 0..16 {
			for note in 0..=127 {
				self.play(MidiEvent {
					channel: ch.into(),
					message: MidiMessage::NoteOff {
						key: note.into(),
						vel: 127.into(),
					},
				});
			}
		}
	}
}

#[cfg(feature = "midir")]
impl Connection for MidiOutputConnection {
	fn play(&mut self, msg: MidiEvent) -> bool {
		let mut buf = Vec::with_capacity(8);
		let _ = msg.write(&mut buf);

		let _ = self.send(&buf);
		true
	}

	fn send_sys_rt(&mut self, msg: SystemRealtime) {
		let mut buf = Vec::with_capacity(8);
		let _ = midly::live::LiveEvent::Realtime(msg).write(&mut buf);
		let _ = self.send(&buf);
	}

	fn send_sys_common(&mut self, msg: SystemCommon<'_>) {
		let mut buf = Vec::with_capacity(8);
		let _ = midly::live::LiveEvent::Common(msg).write(&mut buf);
		let _ = self.send(&buf);
	}
}
