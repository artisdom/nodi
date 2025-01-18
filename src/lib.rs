#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("doc_lib.md")]

mod event;
mod player;
mod learner;
mod sheet;
pub mod timers;

use std::time::Duration;

pub use self::{event::*, player::*, learner::*, sheet::*};
#[cfg(feature = "midir")]
pub use midir;
pub use midly;

use timers::sleep;
use std::f64::consts::E;

/// Used for timing MIDI playback.
pub trait Timer {
	/// Returns the [Duration] that should be slept for.
	///
	/// # Arguments
	/// - `n_ticks`: Number of MIDI ticks to sleep for.
	fn sleep_duration(&mut self, n_ticks: u32) -> Duration;

	/// Changes the timers tempo.
	///
	/// # Arguments
	/// - `tempo`: Represents microseconds per a beat (MIDI quarter note).
	fn change_tempo(&mut self, tempo: u32);

	/// Sleeps given number of ticks.
	/// The provided implementation will sleep the thread  for
	/// `self.sleep_duration(n_ticks)`.
	///
	/// # Notes
	/// The provided implementation will not sleep if
	/// `self.sleep_duration(n_ticks).is_zero()`.
	fn sleep(&mut self, n_ticks: u32) {
		let t = self.sleep_duration(n_ticks);

		if !t.is_zero() {
			sleep(t);
		}
	}

	/// Calculates the length of a track or a slice of [Moment]s.
	///
	/// # Notes
	/// The default implementation modifies `self` if a tempo event is found.
	fn duration(&mut self, moments: &[Moment]) -> Duration {
		let mut counter = Duration::default();
		for moment in moments {
			counter += self.sleep_duration(1);
			for event in &moment.events {
				if let Event::Tempo(val) = event {
					self.change_tempo(*val);
				}
			}
		}
		counter
	}
}

/// Calculates the LED index for a given key.
///
/// # Arguments
/// - `key`: The key value.
///
/// # Returns
/// The calculated LED index.
pub fn get_led_index(key: u8) -> usize {
	let led_offset;

	if key < 56 {
		led_offset = 39;
	} else if key < 69 {
		led_offset = 40;
	} else if key < 93 {
		led_offset = 41;
	} else {
		led_offset = 42;
	}

	key as usize * 2 - led_offset
}

const RAINBOW_FAST_LED: [(u8, u8, u8); 256] =
    [(255, 0, 0), (252, 3, 0), (250, 5, 0), (247, 8, 0), (244, 11, 0), (242, 13, 0), (239, 16, 0), (236, 19, 0),
	 (234, 21, 0), (231, 24, 0), (228, 27, 0), (226, 29, 0), (223, 32, 0), (220, 35, 0), (218, 37, 0), (215, 40, 0),
	 (212, 43, 0), (210, 45, 0), (207, 48, 0), (204, 51, 0), (202, 53, 0), (199, 56, 0), (196, 59, 0), (194, 61, 0),
	 (191, 64, 0), (188, 67, 0), (186, 69, 0), (183, 72, 0), (180, 75, 0), (178, 77, 0), (175, 80, 0), (172, 83, 0),
	 (170, 85, 0), (170, 88, 0), (170, 91, 0), (170, 93, 0), (170, 96, 0), (170, 99, 0), (170, 101, 0), (170, 104, 0),
	 (170, 107, 0), (170, 109, 0), (170, 112, 0), (170, 115, 0), (170, 117, 0), (170, 120, 0), (170, 123, 0), (170, 125, 0),
	 (170, 128, 0), (170, 131, 0), (170, 133, 0), (170, 136, 0), (170, 139, 0), (170, 141, 0), (170, 144, 0), (170, 147, 0),
	 (170, 149, 0), (170, 152, 0), (170, 155, 0), (170, 157, 0), (170, 160, 0), (170, 163, 0), (170, 165, 0), (170, 168, 0),
	 (169, 171, 0), (163, 173, 0), (158, 176, 0), (153, 179, 0), (147, 181, 0), (142, 184, 0), (137, 187, 0), (131, 189, 0),
	 (126, 192, 0), (121, 195, 0), (115, 197, 0), (110, 200, 0), (105, 203, 0), (99, 205, 0), (94, 208, 0), (89, 211, 0),
	 (83, 213, 0), (78, 216, 0), (73, 219, 0), (67, 221, 0), (62, 224, 0), (57, 227, 0), (51, 229, 0), (46, 232, 0),
	 (41, 235, 0), (35, 237, 0), (30, 240, 0), (25, 243, 0), (19, 245, 0), (14, 248, 0), (9, 251, 0), (3, 253, 0),
	 (0, 254, 1), (0, 251, 4), (0, 249, 6), (0, 246, 9), (0, 243, 12), (0, 241, 14), (0, 238, 17), (0, 235, 20),
	 (0, 233, 22), (0, 230, 25), (0, 227, 28), (0, 225, 30), (0, 222, 33), (0, 219, 36), (0, 217, 38), (0, 214, 41),
	 (0, 211, 44), (0, 209, 46), (0, 206, 49), (0, 203, 52), (0, 201, 54), (0, 198, 57), (0, 195, 60), (0, 193, 62),
	 (0, 190, 65), (0, 187, 68), (0, 185, 70), (0, 182, 73), (0, 179, 76), (0, 177, 78), (0, 174, 81), (0, 171, 84),
	 (0, 167, 88), (0, 162, 93), (0, 157, 98), (0, 151, 104), (0, 146, 109), (0, 141, 114), (0, 135, 120), (0, 130, 125),
	 (0, 125, 130), (0, 119, 136), (0, 114, 141), (0, 109, 146), (0, 103, 152), (0, 98, 157), (0, 93, 162), (0, 87, 168),
	 (0, 82, 173), (0, 77, 178), (0, 71, 184), (0, 66, 189), (0, 61, 194), (0, 55, 200), (0, 50, 205), (0, 45, 210),
	 (0, 39, 216), (0, 34, 221), (0, 29, 226), (0, 23, 232), (0, 18, 237), (0, 13, 242), (0, 7, 248), (0, 2, 253),
	 (2, 0, 253), (4, 0, 251), (7, 0, 248), (10, 0, 245), (12, 0, 243), (15, 0, 240), (18, 0, 237), (20, 0, 235),
	 (23, 0, 232), (26, 0, 229), (28, 0, 227), (31, 0, 224), (34, 0, 221), (36, 0, 219), (39, 0, 216), (42, 0, 213),
	 (44, 0, 211), (47, 0, 208), (50, 0, 205), (52, 0, 203), (55, 0, 200), (58, 0, 197), (60, 0, 195), (63, 0, 192),
	 (66, 0, 189), (68, 0, 187), (71, 0, 184), (74, 0, 181), (76, 0, 179), (79, 0, 176), (82, 0, 173), (84, 0, 171),
	 (87, 0, 168), (90, 0, 165), (92, 0, 163), (95, 0, 160), (98, 0, 157), (100, 0, 155), (103, 0, 152), (106, 0, 149),
	 (108, 0, 147), (111, 0, 144), (114, 0, 141), (116, 0, 139), (119, 0, 136), (122, 0, 133), (124, 0, 131), (127, 0, 128),
	 (130, 0, 125), (132, 0, 123), (135, 0, 120), (138, 0, 117), (140, 0, 115), (143, 0, 112), (146, 0, 109), (148, 0, 107),
	 (151, 0, 104), (154, 0, 101), (156, 0, 99), (159, 0, 96), (162, 0, 93), (164, 0, 91), (167, 0, 88), (170, 0, 85),
	 (172, 0, 83), (175, 0, 80), (178, 0, 77), (180, 0, 75), (183, 0, 72), (186, 0, 69), (188, 0, 67), (191, 0, 64),
	 (194, 0, 61), (196, 0, 59), (199, 0, 56), (202, 0, 53), (204, 0, 51), (207, 0, 48), (210, 0, 45), (212, 0, 43),
	 (215, 0, 40), (218, 0, 37), (220, 0, 35), (223, 0, 32), (226, 0, 29), (228, 0, 27), (231, 0, 24), (234, 0, 21),
	 (236, 0, 19), (239, 0, 16), (242, 0, 13), (244, 0, 11), (247, 0, 8), (250, 0, 5), (252, 0, 3), (255, 0, 0)];

/// Calculates the power curve for given values.
///
/// # Arguments
/// - `x`: The input value.
/// - `p`: The power value.
///
/// # Returns
/// The calculated power curve value.
pub fn powercurve(x: f64, p: f64) -> f64 {
	if p == 0.0 {
		return x;
	}

	(E.powf(-p * x) - 1.0) / (E.powf(-p) - 1.0)
}

/// Calculates the color based on the velocity using a rainbow color map.
///
/// # Arguments
/// - `velocity`: The velocity value.
///
/// # Returns
/// A tuple representing the RGB color.
pub fn velocityrainbow_color(velocity: u8) -> (u8, u8, u8) {
	let velocityrainbow_offset = 210;
	let velocityrainbow_scale = 120;
	let velocityrainbow_curve = 0;

	/*
	python code:

        x = int(
                (
                    (
                        255 * powercurve(midi_event.velocity / 127, self.curve / 100)
                        * (self.scale / 100) % 256
                    ) + self.offset
                ) % 256
            )

        return cmap.colormaps[self.colormap][x]
	 */

	let x = powercurve((velocity / 127).into(), (velocityrainbow_curve / 100).into()) * (velocityrainbow_scale / 100) as f64;
	let index = (x as i32 + velocityrainbow_offset) % 256;

	RAINBOW_FAST_LED[index as usize]
}
