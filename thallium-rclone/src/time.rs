//! RFC 3339 timestamps, as rclone writes them.
//!
//! rclone reports `ModTime` as `2026-05-20T11:17:44.123456789Z`. Parsing the
//! handful of shapes it emits is smaller than taking a date library for one
//! field, and it keeps this crate's dependency list to the HTTP surface.

use thallium_fs_core::Timestamp;

/// Parse an RFC 3339 timestamp, returning `None` for anything unrecognised.
#[must_use]
pub fn parse(text: &str) -> Option<Timestamp> {
	let (date, rest) = text.split_once('T')?;
	let mut date = date.splitn(3, '-');
	let year: i64 = date.next()?.parse().ok()?;
	let month: u32 = date.next()?.parse().ok()?;
	let day: u32 = date.next()?.parse().ok()?;

	// Split the zone off before reading the clock: the offset sign cannot be
	// confused with the date separators once the date is already consumed.
	let (clock, offset_seconds) = split_zone(rest)?;
	let mut clock = clock.splitn(3, ':');
	let hour: i64 = clock.next()?.parse().ok()?;
	let minute: i64 = clock.next()?.parse().ok()?;
	let seconds_field = clock.next()?;
	let (whole, fraction) = seconds_field.split_once('.').unwrap_or((seconds_field, ""));
	let second: i64 = whole.parse().ok()?;

	let days = days_from_civil(year, month, day)?;
	let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - i64::from(offset_seconds);
	Some(Timestamp::new(seconds, nanoseconds(fraction)))
}

/// Split the trailing zone designator, returning the clock and its offset in
/// seconds east of UTC.
fn split_zone(rest: &str) -> Option<(&str, i32)> {
	if let Some(clock) = rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z')) {
		return Some((clock, 0));
	}
	// An offset is always the last six characters, `+HH:MM` or `-HH:MM`.
	let boundary = rest.len().checked_sub(6)?;
	let (clock, zone) = rest.split_at(boundary);
	let sign = match zone.as_bytes().first()? {
		b'+' => 1,
		b'-' => -1,
		_ => return None,
	};
	let (hours, minutes) = zone.get(1..)?.split_once(':')?;
	let hours: i32 = hours.parse().ok()?;
	let minutes: i32 = minutes.parse().ok()?;
	Some((clock, sign * (hours * 3_600 + minutes * 60)))
}

/// Nanoseconds from a fractional-second field of any length.
fn nanoseconds(fraction: &str) -> u32 {
	let digits: String = fraction.chars().filter(char::is_ascii_digit).take(9).collect();
	if digits.is_empty() {
		return 0;
	}
	let scale = 10_u32.pow(9 - u32::try_from(digits.len()).unwrap_or(9));
	digits.parse::<u32>().unwrap_or(0).saturating_mul(scale)
}

/// Days since the Unix epoch, by Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i64> {
	if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
		return None;
	}
	let month = i64::from(month);
	let day = i64::from(day);
	let year = if month <= 2 { year - 1 } else { year };
	let era = if year >= 0 { year } else { year - 399 } / 400;
	let year_of_era = year - era * 400;
	let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
	let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
	Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
	use super::parse;

	#[test]
	fn the_epoch_is_zero() {
		let time = parse("1970-01-01T00:00:00Z").expect("parses");
		assert_eq!(time.seconds, 0);
		assert_eq!(time.nanoseconds, 0);
	}

	#[test]
	fn rclone_nanosecond_precision_survives() {
		let time = parse("2026-05-20T11:17:44.123456789Z").expect("parses");
		assert_eq!(time.seconds, 1_779_275_864);
		assert_eq!(time.nanoseconds, 123_456_789);
	}

	#[test]
	fn short_fractions_scale_up() {
		assert_eq!(parse("2020-01-01T00:00:00.5Z").expect("parses").nanoseconds, 500_000_000);
	}

	#[test]
	fn offsets_are_reduced_to_utc() {
		let utc = parse("2026-05-20T11:17:44Z").expect("parses");
		let east = parse("2026-05-20T16:47:44+05:30").expect("parses");
		assert_eq!(utc.seconds, east.seconds);
	}

	#[test]
	fn dates_before_the_epoch_go_negative() {
		assert_eq!(parse("1969-12-31T23:59:59Z").expect("parses").seconds, -1);
	}

	#[test]
	fn nonsense_is_rejected_rather_than_guessed() {
		assert!(parse("").is_none());
		assert!(parse("2026-05-20").is_none());
		assert!(parse("2026-13-01T00:00:00Z").is_none());
	}
}
