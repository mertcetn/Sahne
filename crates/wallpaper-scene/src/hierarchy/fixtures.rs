//! Original typed data only. This fixture is not yet a compiler/renderer input.
use super::*;
pub fn original() -> Hierarchy {
    let rect = |x, y, width, height| Rect {
        x,
        y,
        width,
        height,
    };
    Hierarchy {
        version: VERSION,
        width: 1920,
        height: 1080,
        background: Color([12, 17, 22, 255]),
        resources: vec![],
        layers: vec![
            Layer {
                id: 1,
                parent: 0,
                bounds: rect(80.0, 80.0, 320.0, 240.0),
                content: LayerContent::Group {
                    transform: Transform {
                        translate: [10.0, 20.0],
                        scale: [2.0, 2.0],
                    },
                    opacity: 0.5,
                    clip: true,
                },
            },
            Layer {
                id: 2,
                parent: 1,
                bounds: rect(0.0, 0.0, 100.0, 100.0),
                content: LayerContent::Draw(Content::Rectangle {
                    color: Color([255, 0, 0, 255]),
                }),
            },
            Layer {
                id: 3,
                parent: 1,
                bounds: rect(50.0, 50.0, 100.0, 100.0),
                content: LayerContent::Draw(Content::Rectangle {
                    color: Color([0, 0, 255, 255]),
                }),
            },
            Layer {
                id: 4,
                parent: 0,
                bounds: rect(20.0, 20.0, 200.0, 40.0),
                content: LayerContent::Draw(Content::Text {
                    text: "Sahne / groups".into(),
                    font_size: 24.0,
                    color: Color([255; 4]),
                }),
            },
        ],
    }
}
