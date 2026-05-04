use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PlateAppearance(Vec<Event>);

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Event {
    Pitch(Pitch),
    Runner(Runner),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Pitch {
    #[serde(rename = "@speed")]
    speed: u32,
    #[serde(rename = "@type")]
    r#type: PitchType,
    #[serde(rename = "@outcome")]
    outcome: PitchOutcome,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum PitchType {
    FourSeam,
    TwoSeam,
    Changeup,
    Cutter,
    Curve,
    Slider,
    Knuckle,
    Pitchout,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum PitchOutcome {
    Ball,
    Strike,
    Hit,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Runner {
    #[serde(rename = "@from")]
    from: Base,
    #[serde(rename = "@to")]
    to: Option<Base>,
    #[serde(rename = "@outcome")]
    outcome: RunnerOutcome,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Base {
    First,
    Second,
    Third,
    Home,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum RunnerOutcome {
    Steal,
    Caught,
    PickOff,
}

#[test]
fn main() {
    let document = r#"
        <plate-appearance>
          <pitch speed="95" type="FourSeam" outcome="Ball" />
          <pitch speed="91" type="FourSeam" outcome="Strike" />
          <pitch speed="85" type="Changeup" outcome="Ball" />
          <runner from="First" to="Second" outcome="Steal" />
          <pitch speed="89" type="Slider" outcome="Strike" />
          <pitch speed="88" type="Curve" outcome="Hit" />
        </plate-appearance>"#;
    let plate_appearance: PlateAppearance = from_str(document).unwrap();
    assert_eq!(
        plate_appearance.0[0],
        Event::Pitch(Pitch {
            speed: 95,
            r#type: PitchType::FourSeam,
            outcome: PitchOutcome::Ball
        })
    );
}
