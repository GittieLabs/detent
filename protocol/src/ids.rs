//! USB identity. See "Identity" in `docs/protocol.md` and D-014.

/// Espressif's vendor ID.
pub const VID: u16 = 0x303A;

// PLACEHOLDER: replace with Espressif usb-pids allocation.
// Chosen 2026-11-04 near the top of 0x8000..=0x8FFF, far above the highest
// allocated PID then (0x83EA), so it won't collide before the PR merges.
// Never use TinyUSB or IDF defaults such as 0x4001.
pub const PID: u16 = 0x8FDE;

// PLACEHOLDER: pid.codes' testing pair, until Raspberry Pi assigns a PID
// under its VID 0x2E8A (open question in docs/roadmap.md, D-092).
/// The Raspberry Pi Pico build's vendor ID.
pub const PICO_VID: u16 = 0x1209;
/// The Raspberry Pi Pico build's product ID.
pub const PICO_PID: u16 = 0x0001;

/// A Detent model: a board build with its own USB IDs (M7). The PID names
/// the model (D-004). Its controls come from HELLO (D-094), not from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub vid: u16,
    pub pid: u16,
    /// The `<model name>` in the product string `Detent <model name>`.
    pub name: &'static str,
}

/// Every model Detent knows. The DevKitC build shares the XIAO's IDs for now
/// (open question in docs/roadmap.md), so it reads as the same model.
pub const MODELS: &[Model] = &[
    Model {
        vid: VID,
        pid: PID,
        name: "Dev",
    },
    Model {
        vid: PICO_VID,
        pid: PICO_PID,
        name: "Pico",
    },
];

/// The model with these USB IDs, if Detent knows it.
pub fn model(vid: u16, pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.vid == vid && m.pid == pid)
}

/// Whether these USB IDs are a Detent board.
pub fn is_detent(vid: u16, pid: u16) -> bool {
    model(vid, pid).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_model_is_found_by_its_ids_and_nothing_else() {
        assert_eq!(model(VID, PID).map(|m| m.name), Some("Dev"));
        assert_eq!(model(PICO_VID, PICO_PID).map(|m| m.name), Some("Pico"));
        assert!(model(VID, PICO_PID).is_none(), "VID and PID go together");
        assert!(!is_detent(0x05AC, 0x0256), "an Apple keyboard isn't one");
        // No two models share IDs.
        for (i, a) in MODELS.iter().enumerate() {
            for b in &MODELS[i + 1..] {
                assert!((a.vid, a.pid) != (b.vid, b.pid), "{a:?} and {b:?}");
            }
        }
    }
}
