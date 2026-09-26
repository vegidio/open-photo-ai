//! What this machine is: its processor, its memory and its graphics adapters, read once per process.
//!
//! Reported twice. The collector gets it as resource attributes, so every record, span and data point carries it, and
//! the log file gets it on the session header, the first record under each `---`.

use std::sync::OnceLock;

use rust_sak::o11y::Value;
use rust_sak::sysinfo::{self, GpuInfo};

/// The machine as [`snapshot`] read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hardware {
    /// The processor's model string.
    pub(crate) cpu_model: String,
    /// Logical cores.
    pub(crate) cpu_cores: usize,
    /// Physical memory, in bytes.
    pub(crate) memory: u64,
    /// The graphics adapters, the one with the most dedicated memory first.
    pub(crate) gpus: Vec<GpuInfo>,
}

/// The machine, probed on first use.
///
/// The adapters come from [`gpu::adapters`](crate::gpu::adapters), which is memoised in its own right, so asking here
/// first does not probe them twice.
pub(crate) fn snapshot() -> &'static Hardware {
    static HARDWARE: OnceLock<Hardware> = OnceLock::new();

    HARDWARE.get_or_init(|| {
        let cpu = sysinfo::cpu_info();
        Hardware::new(cpu.name, cpu.cores, sysinfo::memory_info().total, crate::gpu::adapters())
    })
}

impl Hardware {
    /// Assembles a snapshot, ordering `gpus` so the main card comes first.
    pub(crate) fn new(cpu_model: String, cpu_cores: usize, memory: u64, gpus: &[GpuInfo]) -> Self {
        let mut gpus = gpus.to_vec();
        // Stable, so adapters reporting no memory, or the same amount, keep the order the probe found them in. An
        // integrated adapter usually reports none, which puts it behind any discrete card.
        gpus.sort_by_key(|gpu| std::cmp::Reverse(gpu.memory.unwrap_or(0)));

        Self { cpu_model, cpu_cores, memory, gpus }
    }

    /// The pairs the collector receives, keyed so that the collector's dot-to-underscore conversion lands on the
    /// names the Go application's "System info" record used: `cpu_model`, `cpu_cores`, `memory`, `gpu_1_name`,
    /// `gpu_1_memory` and so on. Sizes are in bytes, and an adapter's memory is left out when it is unknown.
    pub(crate) fn attributes(&self) -> Vec<(String, Value)> {
        let mut attributes = vec![
            ("cpu.model".to_string(), Value::from(self.cpu_model.as_str())),
            ("cpu.cores".to_string(), Value::from(self.cpu_cores)),
            ("memory".to_string(), Value::from(self.memory)),
        ];

        for (index, gpu) in self.gpus.iter().enumerate() {
            let n = index + 1;
            attributes.push((format!("gpu.{n}.name"), Value::from(gpu.name.as_str())));

            if let Some(memory) = gpu.memory {
                attributes.push((format!("gpu.{n}.memory"), Value::from(memory)));
            }
        }

        attributes
    }

    /// The adapters as the log file shows them: `NVIDIA GeForce RTX 5090 (31.8 GiB); Intel UHD Graphics 770`, or
    /// `none`.
    pub(crate) fn gpus_summary(&self) -> String {
        if self.gpus.is_empty() {
            return "none".to_string();
        }

        self.gpus
            .iter()
            .map(|gpu| match gpu.memory {
                Some(memory) if memory > 0 => format!("{} ({})", gpu.name, gib(memory)),
                _ => gpu.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// `bytes` in binary gigabytes with one decimal, as a person reading the log file would want it.
pub(crate) fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, memory: Option<u64>) -> GpuInfo {
        GpuInfo { name: name.to_string(), vendor: None, memory }
    }

    #[test]
    fn the_adapter_with_the_most_memory_comes_first_and_ties_keep_their_order() {
        let hardware = Hardware::new(
            "cpu".into(),
            8,
            0,
            &[
                gpu("integrated", None),
                gpu("small", Some(4 << 30)),
                gpu("big", Some(24 << 30)),
                gpu("other", None),
            ],
        );

        let names: Vec<_> = hardware.gpus.iter().map(|gpu| gpu.name.as_str()).collect();
        assert_eq!(names, ["big", "small", "integrated", "other"]);
    }

    #[test]
    fn the_attributes_number_adapters_from_one_and_skip_unknown_memory() {
        let hardware = Hardware::new(
            "AMD Ryzen 9 9950X".into(),
            32,
            64 << 30,
            &[gpu("Intel UHD", None), gpu("NVIDIA GeForce RTX 5090", Some(32 << 30))],
        );

        assert_eq!(
            hardware.attributes(),
            [
                ("cpu.model".to_string(), Value::from("AMD Ryzen 9 9950X")),
                ("cpu.cores".to_string(), Value::Int(32)),
                ("memory".to_string(), Value::Int(64 << 30)),
                ("gpu.1.name".to_string(), Value::from("NVIDIA GeForce RTX 5090")),
                ("gpu.1.memory".to_string(), Value::Int(32 << 30)),
                ("gpu.2.name".to_string(), Value::from("Intel UHD")),
            ]
        );
    }

    #[test]
    fn the_file_summary_names_every_adapter_and_its_memory_when_known() {
        let hardware = Hardware::new("cpu".into(), 1, 0, &[gpu("Intel UHD", Some(0)), gpu("RTX", Some(8 << 30))]);
        assert_eq!(hardware.gpus_summary(), "RTX (8.0 GiB); Intel UHD");

        let none = Hardware::new("cpu".into(), 1, 0, &[]);
        assert_eq!(none.gpus_summary(), "none");
    }

    #[test]
    fn memory_is_shown_in_binary_gigabytes() {
        assert_eq!(gib(16_049_131_520), "14.9 GiB");
        assert_eq!(gib(1 << 30), "1.0 GiB");
    }

    #[test]
    fn the_running_machine_is_probed_once() {
        let first = snapshot();
        assert!(std::ptr::eq(first, snapshot()));
        assert!(first.cpu_cores > 0);
    }
}
