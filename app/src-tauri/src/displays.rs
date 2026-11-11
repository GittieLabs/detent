//! Choosing a display for the dashboard (D-088): by its name, else the main
//! one. Pure; the app passes in what macOS reports.

/// A connected display, in physical pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The display named `wanted` (ignoring case), else the main one (`main`
/// is its index), else the first. `None` only when there are none: a named
/// display that's been unplugged falls back to the main one.
pub fn choose<'a>(all: &'a [Display], wanted: Option<&str>, main: usize) -> Option<&'a Display> {
    let named = wanted.and_then(|w| {
        all.iter().find(|d| {
            d.name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(w.trim()))
        })
    });
    named.or_else(|| all.get(main)).or_else(|| all.first())
}

/// Where a dashboard of `mode` goes on `d`: all of it, or a centred two
/// thirds of it for a window (D-089).
pub fn placement(d: &Display, mode: crate::store::DashboardMode) -> Display {
    match mode {
        crate::store::DashboardMode::Fullscreen => d.clone(),
        crate::store::DashboardMode::Window => {
            let (w, h) = (d.width * 2 / 3, d.height * 2 / 3);
            Display {
                name: d.name.clone(),
                x: d.x + ((d.width - w) / 2) as i32,
                y: d.y + ((d.height - h) / 2) as i32,
                width: w,
                height: h,
            }
        }
    }
}
