use ws2818_rgb_led_spi_driver::adapter_gen::WS28xxAdapter;
use ws2818_rgb_led_spi_driver::adapter_spi::WS28xxSpiAdapter;

#[cfg(feature = "midir")]
use midir::{self, MidiInput, MidiOutputConnection, MidiInputConnection, MidiInputPort};
use midly::{
	live::{SystemCommon, SystemRealtime},
	MidiMessage, Smf, Format,
};

use crate::{
	event::{Event, MidiEvent, Moment}, player::Connection, Sheet, Timer,
	get_led_index,
};

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, Condvar};

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
	pub fn learn(&mut self, sheet: &[Moment], right_hand_track: usize, left_hand_track: usize, learn_track: usize) -> bool {
		let mut counter = 0_u32;
		let adapter = std::sync::Arc::new(std::sync::Mutex::new(
			WS28xxSpiAdapter::new("/dev/spidev0.0").unwrap()
		));

		let (num_leds, r, g, b) = (176, 0, 0, 0);
		let led_data = std::sync::Arc::new(std::sync::Mutex::new(vec![(r, g, b); num_leds]));
		adapter.lock().unwrap().write_rgb(&led_data.lock().unwrap()).unwrap();

		let notes_to_press = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
		let notes_pressed = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
		let condvar_pair = Arc::new((Mutex::new(false), Condvar::new()));

		let midi_in = MidiInput::new("learn_midi").unwrap();
		let in_ports = midi_in.ports();
		let in_port = &in_ports[self.device_no];
		let notes_to_press_clone = std::sync::Arc::clone(&notes_to_press);
		let notes_pressed_clone = std::sync::Arc::clone(&notes_pressed);
		let led_data_clone = std::sync::Arc::clone(&led_data);
		let adapter_clone = std::sync::Arc::clone(&adapter);
		let condvar_pair_clone = condvar_pair.clone();

		let _in_conn = midi_in.connect(in_port, "Casio", move |stamp, message, _| {
			let &(ref condvar_lock, ref condvar) = &*condvar_pair_clone;

			if message[0] != 254 {
				println!("{}: {:?} (len = {})", stamp, message, message.len());

				let key = message[1];
				let index = get_led_index(key);

				match message[0] & 0xF0 {
					0x90 => { // Note on
						notes_pressed_clone.lock().unwrap().insert(key);

						let mut notes_to_press = notes_to_press_clone.lock().unwrap();

						if notes_to_press.contains_key(&key) {
							notes_to_press.insert(key, true); // mark the note as pressed
							*condvar_lock.lock().unwrap() = true;
							condvar.notify_one();
						} else {
							let mut data = led_data_clone.lock().unwrap();
							data[index] = (1, 0, 0); // show red led to show a wrong note pressed
							adapter_clone.lock().unwrap().write_rgb(&data).unwrap();
						}
					}
					0x80 => { // Note off
						notes_pressed_clone.lock().unwrap().remove(&key);

						let mut notes_to_press = notes_to_press_clone.lock().unwrap();

						if notes_to_press.contains_key(&key) {
							notes_to_press.insert(key, false); // mark the note as released
						} else {
							let mut data = led_data_clone.lock().unwrap();
							data[index] = (0, 0, 0); // clear the wrong note led
							adapter_clone.lock().unwrap().write_rgb(&data).unwrap();
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
							let msg_track = msg.track.as_int() as usize;
							let mut play_note = true;

							match msg.message {
								MidiMessage::NoteOn { key, vel } => {
									let index = get_led_index(key.as_int());
									let mut value : u8;
									let mut data = led_data.lock().unwrap();

									if vel == 0 {
										value = 0;
										data[index] = (0, 0, value);
									} else {
										if notes_pressed.lock().unwrap().contains(&key.as_int()) {
											value = 2; // use a deeper color to show the same note needs to be pressed again
										} else {
											value = 1;
										}

										if msg_track == right_hand_track {
											data[index] = (0, value, 0); // Blue
										} else {
											data[index] = (0, 0, value); // Green
										}

										if msg_track == learn_track && key >= 36 && key <= 96 { // support 61 keyborad
											notes_to_press.lock().unwrap().insert(key.as_int(), false);
											play_note = false;
										}
									}
									adapter.lock().unwrap().write_rgb(&data).unwrap();
									println!("NoteOn: key: {}, vel: {}, index: {}, value: {}", key, vel, index, value);
								}
								MidiMessage::NoteOff { key, vel } => {
									let index = get_led_index(key.as_int());
									let mut data = led_data.lock().unwrap();
									data[index] = (0, 0, 0);
									adapter.lock().unwrap().write_rgb(&data).unwrap();

									if msg_track == learn_track && key >= 36 && key <= 96 {
										play_note = false;
									}

									println!("NoteOff: key: {}, vel: {}, index: {}, value: 0", key, vel, index);
								}
								_ => (),
							}

							if play_note {
								if !self.con.play(*msg) {
									return false;
								}
							}

							play_note = true; // reset play_note
						}
						_ => (),
					};
				}

				while !notes_to_press.lock().unwrap().is_empty() {
					if notes_to_press.lock().unwrap().values().all(|&v| v) {
						break;
					}

					// Wait for keys being pressed.
					let &(ref condvar_lock, ref condvar) = &*condvar_pair;
					let mut condvar_lock_state = condvar_lock.lock().unwrap();
					condvar_lock_state = condvar.wait(condvar_lock_state).unwrap();
				}
			}

			counter += 1;
			notes_to_press.lock().unwrap().clear();
		}

		let data_clear = vec![(0, 0, 0); num_leds];
		adapter.lock().unwrap().write_rgb(&data_clear).unwrap();

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