use ws2818_rgb_led_spi_driver::adapter_gen::WS28xxAdapter;
use ws2818_rgb_led_spi_driver::adapter_spi::WS28xxSpiAdapter;

#[cfg(feature = "midir")]
use midir::{self, MidiInput, MidiOutputConnection, MidiInputConnection, MidiInputPort};
use midly::{
	live::{SystemCommon, SystemRealtime},
	MidiMessage,
};

use crate::{
	event::{Event, MidiEvent, Moment},
	Timer,
	player::{Connection},
};

use std::collections::HashMap;

#[doc = include_str!("doc_learner.md")]
pub struct Learner<T: Timer, C: Connection> {
	/// An active midi connection.
	pub con: C,
	pub device_no: usize,
	timer: T,
}

impl<T: Timer, C: Connection> Learner<T, C> {
	/// Creates a new [Learner] with the given [Timer] and
	/// [Connection].
	pub fn new(timer: T, con: C, device_no: usize) -> Self {
		Self { con, device_no, timer }
	}

	/// Changes `self.timer`, returning the old one.
	pub fn set_timer(&mut self, timer: T) -> T {
		std::mem::replace(&mut self.timer, timer)
	}

	/// Learn the given [Moment] slice.
	///
	/// # Notes
	/// The tempo change events are handled by `self.timer` and playing sound by
	/// `self.con`.
	///
	/// Stops learning if [Connection::play] returns `false`.
	/// Returns `true` if the track is played through the end, `false` otherwise.
	pub fn learn(&mut self, sheet: &[Moment], learn_sheet: &[Moment]) -> bool {
		let mut counter = 0_u32;
		let mut adapter = WS28xxSpiAdapter::new("/dev/spidev0.0").unwrap();

		let mut led_offset;
		let (num_leds, r, g, b) = (176, 0, 0, 0);
		let mut data = vec![(r, g, b); num_leds];
		adapter.write_rgb(&data).unwrap();

		let notes_to_press = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));

		let midi_in = MidiInput::new("learn_midi").unwrap();
		let in_ports = midi_in.ports();
		let in_port = &in_ports[self.device_no];
		let notes_to_press_clone = std::sync::Arc::clone(&notes_to_press);

		let _in_conn = midi_in.connect(in_port, "Casio", move |stamp, message, _| {
			if message[0] != 254 {
                println!("{}: {:?} (len = {})", stamp, message, message.len());

				let key = message[1];

				match message[0] {
					144 => { // Note on
						let mut notes_to_press = notes_to_press_clone.lock().unwrap();
						match notes_to_press.get(&key) {
							Some(&_value) => { notes_to_press.insert(key, true); },
							_ => {},
						}
					}
					128 => { // Note off
						let mut notes_to_press = notes_to_press_clone.lock().unwrap();
						match notes_to_press.get(&key) {
							Some(&_value) => { notes_to_press.insert(key, false); },
							_ => {},
						}
                    }
                    _ => (),
                }
			}
		}, ());

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

									if key < 56 {
										led_offset = 39;
									} else if key < 69 {
										led_offset = 40;
									} else if key < 93 {
										led_offset = 41;
									} else {
										led_offset = 42;
									}

									let index = key.as_int() as usize * 2 - led_offset;
									let mut value : u8;

									if vel == 0 {
										value = 0;
									} else {
										// value = ((vel.as_int() as f32 / 127.0) * (100.0 / 10.0)) as u8;
										value = 1;

										if value < 1 {
											value = 1;
										}

										notes_to_press.lock().unwrap().insert(key.as_int(), false);
									}

									data[index] = (0, 0, value);
									adapter.write_rgb(&data).unwrap();

									println!("NoteOn: key: {}, vel: {}, index: {}, value: {}", key, vel, index, value);
								}
								MidiMessage::NoteOff { key, vel } => {

									if key < 56 {
										led_offset = 39;
									} else if key < 69 {
										led_offset = 40;
									} else if key < 93 {
										led_offset = 41;
									} else {
										led_offset = 42;
									}

									let index = key.as_int() as usize * 2 - led_offset;

									data[index] = (0, 0, 0);
									adapter.write_rgb(&data).unwrap();
									println!("NoteOff: key: {}, vel: {}, index: {}, value: 0", key, vel, index);
								}
								_ => (),
							}

							// if !self.con.play(*msg) {
							// 	return false;
							// }
						}
						_ => (),
					};
				}

				while !notes_to_press.lock().unwrap().is_empty() {
					if notes_to_press.lock().unwrap().values().all(|&v| v) {
						break;
					}
				}
			}

			counter += 1;
			notes_to_press.lock().unwrap().clear();
		}

		let data_clear = vec![(0, 0, 0); num_leds];
		adapter.write_rgb(&data_clear).unwrap();

		true
	}
}
/*

/// Any type that can play sound, given a [MidiEvent].
///
/// This trait is implemented for midir::MidiOutputConnection, if the `midir`
/// feature is enabled.
pub trait Connection {
	/// Given a [MidiEvent], plays the message.
	///
	/// If this function returns `false`, [Learner::play] will stop playing and return.
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
*/
pub trait InputConnection {
	// fn read(&mut self) -> Option<MidiEvent>;
}

#[cfg(feature = "midir")]
impl InputConnection for MidiInputConnection<()> {
	// fn read(&mut self) -> Option<MidiEvent> {
	// 	let mut buf = [0; 3];
	// 	let _ = self.read(&mut buf);
	// 	MidiEvent::try_from(buf).ok()
	// }
}