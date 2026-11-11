//! Choosing the dashboard's display (M6): by name, else the main display.

use detent_lib::displays::{choose, Display};

fn d(name: &str, x: i32, w: u32) -> Display {
    Display {
        name: Some(name.into()),
        x,
        y: 0,
        width: w,
        height: 1000,
    }
}

#[test]
fn the_named_display_is_chosen_ignoring_case() {
    let all = [
        d("Built-in Retina Display", 0, 1512),
        d("Studio Display", 1512, 2560),
    ];
    assert_eq!(choose(&all, Some("studio display"), 0), Some(&all[1]));
}

#[test]
fn a_missing_or_unnamed_choice_falls_back_to_the_main_display() {
    let all = [
        d("Built-in Retina Display", 0, 1512),
        d("Studio Display", 1512, 2560),
    ];
    // Unplugged: the named display is gone.
    assert_eq!(choose(&all[..1], Some("Studio Display"), 0), Some(&all[0]));
    assert_eq!(choose(&all, None, 0), Some(&all[0]));
    // The main display's index comes from the caller; a bad one still finds one.
    assert_eq!(choose(&all, None, 7), Some(&all[0]));
    assert_eq!(choose(&[], None, 0), None);
}

#[test]
fn full_screen_covers_the_display_and_a_window_sits_centred() {
    use detent_lib::displays::placement;
    use detent_lib::store::DashboardMode;
    let studio = Display {
        name: Some("Studio Display".into()),
        x: 1512,
        y: -200,
        width: 3000,
        height: 1800,
    };
    assert_eq!(placement(&studio, DashboardMode::Fullscreen), studio);
    let w = placement(&studio, DashboardMode::Window);
    assert_eq!((w.width, w.height), (2000, 1200));
    assert_eq!(
        (w.x, w.y),
        (1512 + 500, -200 + 300),
        "centred, on that display"
    );
}
