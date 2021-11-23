use pan_tilt_format::ServerToDevice;

#[derive(Debug)]
pub enum Action {
    SendMessage(ServerToDevice),
    PrintErr(&'static str),
}

pub struct Cmd {}

impl Cmd {
    pub fn new() -> Self {
        Self {}
    }

    pub fn parse_line(&mut self, line: &str) -> Action {
        use Action::*;
        dbg!(line);

        let mut parts = line.split(' ');
        let first = parts.next();
        if let Some("pan") = first {
            if let Some(Ok(degrees)) = parts.next().map(|p| p.parse::<f32>()) {
                return SendMessage(ServerToDevice {
                    pan_degrees: Some(degrees),
                    tilt_degrees: None,
                });
            }
        }
        if let Some("tilt") = first {
            if let Some(Ok(degrees)) = parts.next().map(|p| p.parse::<f32>()) {
                return SendMessage(ServerToDevice {
                    tilt_degrees: Some(degrees),
                    pan_degrees: None,
                });
            }
        }

        PrintErr("Error parsing command")
    }
}
