use iced::{
    Length, Alignment, Border, Element, Theme, Task,
    color,
    widget::{column, container, text, row, scrollable, rule},
};
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows::{
    core::{PCWSTR, PWSTR, Result},
    Win32::{
        Foundation::{GetLastError, ERROR_MORE_DATA},
        System::Services::*,
    },
};


#[derive(Debug, Clone)]
pub enum Message {
    Noop,
}


struct App {
     services: Result<Vec<ServiceBasic>>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                services: enumerate_services_basic(),
            },
            Task::none()
        )
    }

    fn title(&self) -> String {
        String::from("Windows Service Manager")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn window_settings() -> iced::window::Settings {
        iced::window::Settings {
            size: iced::Size::new(1500.0, 850.0),
            ..iced::window::Settings::default()
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Noop => {}
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let mut service_count: usize = 0;
        let services: Element<Message, Theme, iced::Renderer> = match self.services.clone() {
            Ok(services) => {
                service_count = services.len();
                println!("Service Count: {:?}", service_count);
                let services = services.iter().map(|service|
                    row![
                        column![text(service.name.clone())].width(Length::Fill),
                        column![text(service.display_name.clone())].width(Length::Fill),
                        column![text(service_state_to_string(service.state))].width(Length::Fill),
                        column![text(service.process_id)].width(Length::Fill)
                    ].into(),
                );
                column(services).into()
            }
            Err(_) => { column!(text("Unable to retrieve Services.")).into() }
        };

        container(
            column![
                text("Windows Service Viewer")
                    .size(25),
                row![
                    column![text("Name").size(18).color(self.theme().extended_palette().primary.strong.color)].width(Length::Fill),
                    column![text("Display Name").size(18).color(self.theme().extended_palette().primary.strong.color)].width(Length::Fill),
                    column![text("State").size(18).color(self.theme().extended_palette().primary.strong.color)].width(Length::Fill),
                    column![text("Process Id").size(18).color(self.theme().extended_palette().primary.strong.color)].width(Length::Fill)
                ],
                rule::horizontal(2),
                scrollable(services).height(Length::Fill),
                row![text(format!("Service Count: {:?}", service_count)).color(self.theme().extended_palette().secondary.base.color)].width(Length::Fill),
            ]
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(10.0)
        ).into()
    }

}

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .window(App::window_settings())
        .theme(App::theme)
        .title(App::title)
        .run()
}



fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

unsafe fn pwstr_to_string(p: PWSTR) -> String {
    if p.0.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *p.0.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len))
}

#[derive(Debug, Clone)]
pub struct ServiceBasic {
    pub name: String,
    pub display_name: String,
    pub state: SERVICE_STATUS_CURRENT_STATE,
    pub process_id: u32,
}

pub fn enumerate_services_basic() -> Result<Vec<ServiceBasic>> {
    unsafe {
        let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE)?;

        let service_type = SERVICE_WIN32 | SERVICE_DRIVER; // “all” practical types
        let service_state = SERVICE_STATE_ALL;

        let mut bytes_needed: u32 = 0;
        let mut services_returned: u32 = 0;
        let mut resume_handle: u32 = 0;

        // Probe call: expect ERROR_MORE_DATA
        let probe = EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            service_type,
            service_state,
            None,
            &mut bytes_needed,
            &mut services_returned,
            Some(&mut resume_handle as *mut u32),
            PCWSTR::null(),
        );

        match probe {
            Ok(()) => {
                // It succeeded with no buffer. If nothing needed, there are no services to return.
                if bytes_needed == 0 || services_returned == 0 {
                    let _ = CloseServiceHandle(scm);
                    return Ok(Vec::new());
                }
                // Otherwise continue and allocate.
            }
            Err(e) => {
                // If it's not the expected "need more data", bail with the error.
                if GetLastError() != ERROR_MORE_DATA {
                    let _ = CloseServiceHandle(scm);
                    return Err(e);
                }
            }
        }

        let mut buffer = vec![0u8; bytes_needed as usize];

        // Real call
        services_returned = 0;
        resume_handle = 0;

        EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            service_type,
            service_state,
            Some(buffer.as_mut_slice()),
            &mut bytes_needed,
            &mut services_returned,
            Some(&mut resume_handle as *mut u32),
            PCWSTR::null(),
        )?;

        let base = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
        let entries = std::slice::from_raw_parts(base, services_returned as usize);

        let mut out = Vec::with_capacity(entries.len());
        for e in entries {
            out.push(ServiceBasic {
                name: pwstr_to_string(e.lpServiceName),
                display_name: pwstr_to_string(e.lpDisplayName),
                state: e.ServiceStatusProcess.dwCurrentState,
                process_id: e.ServiceStatusProcess.dwProcessId,
            });
        }

        let _ = CloseServiceHandle(scm);
        Ok(out)
    }
}

pub fn service_state_to_str(state: SERVICE_STATUS_CURRENT_STATE) -> &'static str {
    match state {
        SERVICE_STOPPED => "Stopped",
        SERVICE_START_PENDING => "Start pending",
        SERVICE_STOP_PENDING => "Stop pending",
        SERVICE_RUNNING => "Running",
        SERVICE_CONTINUE_PENDING => "Continue pending",
        SERVICE_PAUSE_PENDING => "Pause pending",
        SERVICE_PAUSED => "Paused",
        _ => "Unknown",
    }
}

pub fn service_state_to_string(state: SERVICE_STATUS_CURRENT_STATE) -> String {
    service_state_to_str(state).to_string()
}