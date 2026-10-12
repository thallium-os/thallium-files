//! Drives and partitions that can be mounted from the sidebar.
//!
//! `lsblk` (always present) says what exists; `udisksctl` (from udisks2)
//! mounts, unmounts and powers off, so polkit decides who may, exactly as
//! for every other desktop file manager. Each action runs on a thread of
//! its own and is collected later through [`Pending`].

use std::{path::PathBuf, process::Command, sync::{Arc, Mutex}, thread};

/// Partition types udisks marks to be ignored: EFI system, BIOS boot,
/// Microsoft reserved and Windows recovery, in GPT and MBR forms.
const HIDDEN_TYPES: &[&str] = &[
	"c12a7328-f81f-11d2-ba4b-00a0c93ec93b",
	"21686148-6449-6e6f-744e-656564454649",
	"e3c9e316-0b5c-4db8-817d-f92df00215ae",
	"de94bba4-06d1-4d40-a16a-bfd50179d6ac",
	"0xef",
	"0x27",
];

/// Mount points that belong to the system, never offered for unmounting.
const SYSTEM_MOUNTS: &[&str] =
	&["/", "/boot", "/boot/efi", "/efi", "/home", "/usr", "/var", "/tmp"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Drive {
	/// Block device, such as `/dev/sdb1`.
	pub device:    String,
	/// The whole disk it is part of, which is what gets powered off.
	pub disk:      String,
	pub label:     String,
	pub size:      String,
	pub mount:     Option<PathBuf>,
	/// Removable or hot-plugged, so it can be ejected.
	pub ejectable: bool,
}

/// Mountable filesystems that are not part of the running system.
pub fn list() -> Vec<Drive> {
	let Ok(output) = Command::new("lsblk")
		.args(["-P", "-o", "PATH,PKNAME,LABEL,SIZE,FSTYPE,MOUNTPOINT,RM,HOTPLUG,TYPE,MODEL,PARTTYPE"])
		.output()
	else {
		return Vec::new();
	};
	parse(&String::from_utf8_lossy(&output.stdout))
}

fn parse(text: &str) -> Vec<Drive> {
	let mut drives = Vec::new();
	for line in text.lines() {
		let field = |key: &str| value(line, key).unwrap_or_default();
		let kind = field("TYPE");
		let fstype = field("FSTYPE");
		if !matches!(kind.as_str(), "part" | "disk" | "crypt")
			|| fstype.is_empty()
			|| fstype == "swap"
			|| HIDDEN_TYPES.contains(&field("PARTTYPE").to_lowercase().as_str())
		{
			continue;
		}
		let mount = Some(field("MOUNTPOINT")).filter(|mount| !mount.is_empty());
		if mount
			.as_deref()
			.is_some_and(|mount| SYSTEM_MOUNTS.contains(&mount) || mount.starts_with('['))
		{
			continue;
		}
		let ejectable = field("RM") == "1" || field("HOTPLUG") == "1";
		let pkname = field("PKNAME");
		let device = field("PATH");
		let label = [field("LABEL"), field("MODEL")]
			.into_iter()
			.find(|text| !text.trim().is_empty())
			.unwrap_or_else(|| format!("{} volume", field("SIZE")));
		drives.push(Drive {
			disk: if pkname.is_empty() { device.clone() } else { format!("/dev/{pkname}") },
			device,
			label: label.trim().to_owned(),
			size: field("SIZE"),
			mount: mount.map(PathBuf::from),
			ejectable,
		});
	}
	drives
}

/// The value of `KEY="..."` in one line of `lsblk -P`, with its `\xNN`
/// escapes decoded.
fn value(line: &str, key: &str) -> Option<String> {
	// The key must start a field, so `TYPE` is not found inside `FSTYPE`.
	let needle = format!("{key}=\"");
	let start = line
		.match_indices(&needle)
		.find(|(at, _)| *at == 0 || line.as_bytes()[at - 1] == b' ')
		.map(|(at, _)| at + needle.len())?;
	let rest = &line[start..];
	let raw = &rest[..rest.find('"')?];
	let bytes = raw.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut at = 0;
	while at < bytes.len() {
		if bytes[at] == b'\\'
			&& bytes.get(at + 1) == Some(&b'x')
			&& let Some(hex) = raw.get(at + 2..at + 4)
			&& let Ok(byte) = u8::from_str_radix(hex, 16)
		{
			out.push(byte);
			at += 4;
			continue;
		}
		out.push(bytes[at]);
		at += 1;
	}
	Some(String::from_utf8_lossy(&out).into_owned())
}

/// What a drive action finished with: where a mount landed, or why it failed.
pub type Outcome = Result<Option<PathBuf>, String>;

/// A drive action in flight. `None` while it runs, then its outcome once.
#[derive(Clone, Default)]
pub struct Pending(Arc<Mutex<Option<Outcome>>>);

impl Pending {
	pub fn take(&self) -> Option<Outcome> { self.0.lock().ok()?.take() }

	fn run(args: Vec<String>, mounting: bool) -> Self {
		let pending = Self::default();
		let slot = Arc::clone(&pending.0);
		thread::spawn(move || {
			let outcome = udisksctl(&args).map(|said| if mounting { mounted_at(&said) } else { None });
			if let Ok(mut slot) = slot.lock() {
				*slot = Some(outcome);
			}
		});
		pending
	}
}

pub fn mount(drive: &Drive) -> Pending {
	Pending::run(vec!["mount".into(), "-b".into(), drive.device.clone()], true)
}

pub fn unmount(drive: &Drive) -> Pending {
	Pending::run(vec!["unmount".into(), "-b".into(), drive.device.clone()], false)
}

/// Unmount if needed, then power the disk off so it can be pulled out.
pub fn eject(drive: &Drive) -> Pending {
	let pending = Pending::default();
	let slot = Arc::clone(&pending.0);
	let drive = drive.clone();
	thread::spawn(move || {
		let unmounted = match drive.mount {
			Some(_) => udisksctl(&["unmount".into(), "-b".into(), drive.device.clone()]).map(|_| ()),
			None => Ok(()),
		};
		let outcome = unmounted
			.and_then(|()| udisksctl(&["power-off".into(), "-b".into(), drive.disk.clone()]))
			.map(|_| None);
		if let Ok(mut slot) = slot.lock() {
			*slot = Some(outcome);
		}
	});
	pending
}

fn udisksctl(args: &[String]) -> Result<String, String> {
	let output = Command::new("udisksctl").args(args).output().map_err(|error| {
		if error.kind() == std::io::ErrorKind::NotFound {
			"Mounting drives needs udisks2, which is not installed".to_owned()
		} else {
			error.to_string()
		}
	})?;
	if output.status.success() {
		Ok(String::from_utf8_lossy(&output.stdout).into_owned())
	} else {
		Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
	}
}

/// The folder in `udisksctl mount`'s "Mounted /dev/sdb1 at /media/x/LABEL".
fn mounted_at(said: &str) -> Option<PathBuf> {
	said.trim().split_once(" at ").map(|(_, path)| PathBuf::from(path.trim().trim_end_matches('.')))
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use super::{mounted_at, parse};

	#[test]
	fn system_and_swap_are_left_out() {
		let text = concat!(
			"PATH=\"/dev/nvme0n1\" PKNAME=\"\" LABEL=\"\" SIZE=\"476G\" FSTYPE=\"\" MOUNTPOINT=\"\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"disk\" MODEL=\"Samsung\"\n",
			"PATH=\"/dev/nvme0n1p2\" PKNAME=\"nvme0n1\" LABEL=\"\" SIZE=\"60G\" FSTYPE=\"ext4\" MOUNTPOINT=\"/\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"part\" MODEL=\"\"\n",
			"PATH=\"/dev/nvme0n1p3\" PKNAME=\"nvme0n1\" LABEL=\"\" SIZE=\"8G\" FSTYPE=\"swap\" MOUNTPOINT=\"[SWAP]\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"part\" MODEL=\"\"\n",
			"PATH=\"/dev/nvme0n1p6\" PKNAME=\"nvme0n1\" LABEL=\"SAMSUNG_REC\" SIZE=\"20G\" FSTYPE=\"vfat\" MOUNTPOINT=\"\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"part\" MODEL=\"\" PARTTYPE=\"de94bba4-06d1-4d40-a16a-bfd50179d6ac\"\n",
			"PATH=\"/dev/nvme0n1p1\" PKNAME=\"nvme0n1\" LABEL=\"\" SIZE=\"800M\" FSTYPE=\"vfat\" MOUNTPOINT=\"\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"part\" MODEL=\"\" PARTTYPE=\"c12a7328-f81f-11d2-ba4b-00a0c93ec93b\"\n",
			"PATH=\"/dev/nvme0n1p8\" PKNAME=\"nvme0n1\" LABEL=\"DATA\" SIZE=\"100G\" FSTYPE=\"ntfs\" MOUNTPOINT=\"\" RM=\"0\" HOTPLUG=\"0\" TYPE=\"part\" MODEL=\"\" PARTTYPE=\"ebd0a0a2-b9e5-4433-87c0-68b6b72699c7\"\n",
			"PATH=\"/dev/sdb1\" PKNAME=\"sdb\" LABEL=\"My\\x20Stick\" SIZE=\"29G\" FSTYPE=\"exfat\" MOUNTPOINT=\"/media/u/My Stick\" RM=\"1\" HOTPLUG=\"1\" TYPE=\"part\" MODEL=\"\"\n",
		);
		let drives = parse(text);
		assert_eq!(drives.len(), 2, "EFI and recovery partitions are hidden: {drives:?}");
		assert_eq!(drives[0].label, "DATA");
		assert!(!drives[0].ejectable && drives[0].mount.is_none());
		assert_eq!(drives[1].label, "My Stick");
		assert_eq!(drives[1].disk, "/dev/sdb");
		assert_eq!(drives[1].mount.as_deref(), Some("/media/u/My Stick".as_ref()));
		assert!(drives[1].ejectable);
	}

	#[test]
	fn the_mount_folder_is_read_from_udisksctl() {
		assert_eq!(
			mounted_at("Mounted /dev/sdb1 at /media/u/STICK\n"),
			Some(PathBuf::from("/media/u/STICK"))
		);
		assert_eq!(
			mounted_at("Mounted /dev/sdb1 at /media/u/STICK.\n"),
			Some(PathBuf::from("/media/u/STICK"))
		);
	}
}
