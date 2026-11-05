//! Native PS/2 gestures checked against explicitly chosen canonical commands.
//! This does not call the vessel's gesture adapter to predict its own behavior.
use super::*;
use anomalous_desktop_compositor::{appearance_controls_visible, chrome_bounds, chrome_target};
use anomalous_desktop_workspace::{
    appearance_choice, client_endpoint, client_grants, client_origin, demo_content, density_mode,
    desktop, desktop_command as command, desktop_control, launcher_choice, layout_mode, theme_mode,
    theme_settings, update_ticket, window_id, window_rect, workspace_id,
};

struct pointer_driver<'a> {
    child: &'a mut Child,
    monitor: ChildStdin,
    files: &'a captures,
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    records: Vec<String>,
    frames: usize,
    buttons: u8,
    pages: u64,
    deadline: Instant,
}

fn exact_records(text: &str, records: &[String]) -> bool {
    text.len() <= transcript_limit && text == records.join("\r\n") + "\r\n"
}

fn verify_final_records(
    profile: workspace_profile,
    transcript: &str,
    records: &[String],
) -> Result<(), String> {
    let normalized = profile
        .normalize(transcript)?
        .ok_or("pointer smoke final CPU lifecycle is incomplete")?;
    if !exact_records(&normalized, records) {
        return Err("pointer smoke final transcript changed after withdrawal".into());
    }
    Ok(())
}

fn wait_exact(
    child: &mut Child,
    profile: workspace_profile,
    log: &Path,
    records: &[String],
    deadline: Instant,
) -> Result<(), String> {
    let expected = records.join("\r\n") + "\r\n";
    if expected.len() > transcript_limit {
        return Err("pointer smoke expected lifecycle exceeds bound".into());
    }
    loop {
        ensure_child_running(child)?;
        let text = read_bounded_optional_utf8_file(log, transcript_limit)?;
        let Some(text) = profile.normalize(&text)? else {
            if Instant::now() >= deadline {
                return Err("CPU lifecycle record deadline exceeded".into());
            }
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        if exact_records(&text, records) {
            return Ok(());
        }
        if !expected.starts_with(&text) {
            return Err(
                "pointer smoke received an unexpected lifecycle record; captures removed".into(),
            );
        }
        if Instant::now() >= deadline {
            return Err("pointer smoke lifecycle deadline exceeded".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

impl<'a> pointer_driver<'a> {
    fn new(child: &'a mut Child, files: &'a captures, pages: u64) -> Result<Self, String> {
        let monitor = child.stdin.take().ok_or("pointer smoke monitor missing")?;
        let deadline = Instant::now() + Duration::from_secs(300);
        wait_for(child, &files.log_part, &frame_record(1), deadline)?;
        let text = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
        files.profile.verify(&text, pages, 1, None)?;
        let text = files
            .profile
            .normalize(&text)?
            .ok_or("incomplete CPU readiness")?;
        let records = text
            .strip_suffix("\r\n")
            .unwrap()
            .split("\r\n")
            .map(str::to_owned)
            .collect();
        let (mut model, control, ticket) = desktop::new_demo(1024, 768).map_err(|e| e.code())?;
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .map_err(|e| e.code())?;
        let mut result = Self {
            child,
            monitor,
            files,
            model,
            control,
            human,
            records,
            frames: 1,
            buttons: 0,
            pages,
            deadline,
        };
        result.finish_frame(ticket)?;
        Ok(result)
    }

    fn finish_frame(&mut self, ticket: update_ticket) -> Result<Vec<u8>, String> {
        wait_exact(
            self.child,
            self.files.profile,
            &self.files.log_part,
            &self.records,
            self.deadline,
        )?;
        let expected = reference_frame(&self.model.candidate(&ticket).map_err(|e| e.code())?)?;
        let actual = capture_frame(
            self.child,
            &mut self.monitor,
            self.files,
            &expected,
            self.deadline,
        )
        .map_err(|error| format!("pointer frame {}: {error}", self.frames))?;
        self.model
            .begin(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .attest_presented(&self.control, &ticket)
            .map_err(|e| e.code())?;
        self.model
            .publish(&self.control, ticket)
            .map_err(|_| "publish pointer host reference")?;
        Ok(actual)
    }

    fn view(&self) -> Result<anomalous_desktop_workspace::render_view<'_>, String> {
        self.model.observe(&self.human).map_err(|e| e.code().into())
    }

    fn packet(
        &mut self,
        monitor_command: &str,
        x: u32,
        y: u32,
        actions: &[command],
    ) -> Result<(), String> {
        let mut commands = vec![
            command::set_pointer { x, y },
            command::set_pointer_buttons(self.buttons),
        ];
        commands.extend_from_slice(actions);
        let revision = self.view()?.revision();
        let ticket = self
            .model
            .prepare_batch(&self.human, revision, &commands)
            .map_err(|e| e.code())?;
        self.frames += 1;
        self.records.push(frame_record(self.frames));
        writeln!(self.monitor, "{monitor_command}")
            .and_then(|()| self.monitor.flush())
            .map_err(|_| "send physical pointer input")?;
        self.finish_frame(ticket)?;
        Ok(())
    }

    fn motion(&mut self, dx: i32, dy: i32, actions: &[command]) -> Result<(), String> {
        if dx.abs() > 64 || dy.abs() > 64 || (dx == 0 && dy == 0) {
            return Err("pointer smoke motion must fit one nonempty bounded packet".into());
        }
        let position = self.view()?.pointer();
        let x = (i64::from(position.x()) + i64::from(dx)).clamp(0, 1023) as u32;
        let y = (i64::from(position.y()) + i64::from(dy)).clamp(0, 767) as u32;
        self.packet(&format!("mouse_move {dx} {dy}"), x, y, actions)
    }

    fn move_to(&mut self, x: u32, y: u32) -> Result<(), String> {
        loop {
            let position = self.view()?.pointer();
            let dx = (i64::from(x) - i64::from(position.x())).clamp(-64, 64) as i32;
            let dy = (i64::from(y) - i64::from(position.y())).clamp(-64, 64) as i32;
            if dx == 0 && dy == 0 {
                return Ok(());
            }
            self.motion(dx, dy, &[])?;
        }
    }

    fn button(&mut self, buttons: u8, actions: &[command]) -> Result<(), String> {
        self.buttons = buttons;
        let position = self.view()?.pointer();
        self.packet(
            &format!("mouse_button {buttons}"),
            position.x(),
            position.y(),
            actions,
        )
    }

    fn target(&mut self, target: chrome_target) -> Result<(), String> {
        let rect = chrome_bounds(&self.view()?, target).ok_or("pointer smoke target is absent")?;
        self.move_to(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    fn click(&mut self, target: chrome_target, actions: &[command]) -> Result<(), String> {
        self.target(target)?;
        self.button(1, &[command::arm_pointer(target)])?;
        self.button(0, actions)
    }

    fn capture(&mut self) -> Result<Vec<u8>, String> {
        wait_exact(
            self.child,
            self.files.profile,
            &self.files.log_part,
            &self.records,
            self.deadline,
        )?;
        let expected = reference_frame(&self.view()?)?;
        capture_frame(
            self.child,
            &mut self.monitor,
            self.files,
            &expected,
            self.deadline,
        )
    }

    fn key(&mut self, key: &str, action: command) -> Result<(), String> {
        let ticket = self
            .model
            .prepare(&self.human, self.view()?.revision(), action)
            .map_err(|e| e.code())?;
        self.frames += 1;
        self.records.push(frame_record(self.frames));
        type_monitor_named_key(&mut self.monitor, key)?;
        self.finish_frame(ticket)?;
        Ok(())
    }

    fn ignored_key(&mut self, key: &str) -> Result<(), String> {
        type_monitor_named_key(&mut self.monitor, key)?;
        // Allow the make and break records to reach the native adapter. The
        // next accepted frame also checks the unchanged closed transcript.
        thread::sleep(Duration::from_millis(100));
        wait_exact(
            self.child,
            self.files.profile,
            &self.files.log_part,
            &self.records,
            self.deadline,
        )
    }

    fn withdraw(&mut self) -> Result<(), String> {
        if matches!(
            self.files.profile,
            workspace_profile::owned | workspace_profile::owned_cpu
        ) {
            self.records.push(owned_workspace::released_record(
                "desktop",
                "task_exit",
                self.frames,
            ));
        }
        let tail = fallback_records(self.frames, "task_exit");
        self.records.extend_from_slice(&tail[..3]);
        type_monitor_named_key(&mut self.monitor, "esc")?;
        wait_exact(
            self.child,
            self.files.profile,
            &self.files.log_part,
            &self.records,
            self.deadline,
        )?;
        self.records.extend_from_slice(&tail[3..]);
        type_monitor_line(&mut self.monitor, "version")?;
        wait_exact(
            self.child,
            self.files.profile,
            &self.files.log_part,
            &self.records,
            self.deadline,
        )
    }

    fn restart(&mut self) -> Result<(), String> {
        let mut restart = restart_records(self.pages, 2);
        if matches!(
            self.files.profile,
            workspace_profile::owned | workspace_profile::owned_cpu
        ) {
            restart.insert(3, owned_workspace::prepared_record("desktop"));
        }
        self.records.extend(restart);
        let (model, control, ticket) = desktop::new_demo(1024, 768).map_err(|e| e.code())?;
        self.model = model;
        self.control = control;
        self.human = self
            .model
            .bind_client(&self.control, client_origin::human, client_grants::all)
            .map_err(|e| e.code())?;
        self.frames = 1;
        self.buttons = 0;
        type_monitor_line(&mut self.monitor, "desktop")?;
        self.finish_frame(ticket)?;
        if self.view()?.pointer().visible()
            || self.view()?.pointer_buttons() != 0
            || self.view()?.pointer_armed().is_some()
            || self.view()?.theme() != theme_mode::light
            || self.view()?.theme_settings() != theme_settings::initial
            || self
                .view()?
                .all_windows()
                .any(|window| window.content() == demo_content::appearance)
        {
            return Err("fresh pointer workspace inherited retired state".into());
        }
        Ok(())
    }
}

fn restart_records(pages: u64, generation: u32) -> Vec<String> {
    vec![
        r#"{"schema":1,"event":"bootstrap_shell.command.received","level":"info","sequence":2,"kind":"desktop"}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.result","level":"info","sequence":2,"kind":"desktop","returned":true}"#.into(),
        r#"{"schema":1,"event":"bootstrap_shell.command.completed","level":"info","sequence":2,"kind":"desktop","status":"ok"}"#.into(),
        pointer_record(generation), ready_record(pages), frame_record(1),
    ]
}

fn first_window(driver: &pointer_driver<'_>) -> Result<window_id, String> {
    driver
        .view()?
        .all_windows()
        .find(|window| window.id().slot() == 0)
        .map(|window| window.id())
        .ok_or("public welcome window missing".into())
}

fn appearance_window(driver: &pointer_driver<'_>) -> Result<window_id, String> {
    let view = driver.view()?;
    let mut windows = view
        .all_windows()
        .filter(|window| window.content() == demo_content::appearance);
    let window = windows.next().ok_or("public Appearance window missing")?;
    if windows.next().is_some() || !window.focused() || window.minimized() {
        return Err("Appearance singleton was duplicated or not restored and focused".into());
    }
    Ok(window.id())
}

fn appearance_selection(
    driver: &pointer_driver<'_>,
    window: window_id,
    choice: appearance_choice,
) -> Result<(), String> {
    if driver
        .view()?
        .window(window)
        .and_then(|window| window.appearance_selection())
        != Some(choice)
    {
        return Err("Appearance keyboard selection differs from its canonical command".into());
    }
    Ok(())
}

fn appearance_click(
    driver: &mut pointer_driver<'_>,
    window: window_id,
    choice: appearance_choice,
) -> Result<(), String> {
    driver.click(
        chrome_target::appearance_choice(window, choice),
        &[command::activate_appearance { window, choice }],
    )?;
    appearance_selection(driver, window, choice)
}

fn launcher_click(driver: &mut pointer_driver<'_>, choice: launcher_choice) -> Result<(), String> {
    driver.click(
        chrome_target::launcher_item(choice),
        &[
            command::select_launcher(choice),
            command::activate_launcher {
                choice,
                graphics: None,
            },
        ],
    )
}

fn launcher_sequence(driver: &mut pointer_driver<'_>) -> Result<Vec<u8>, String> {
    let count = driver.view()?.all_windows().count();
    let focused = driver.view()?.focused_window().unwrap().id();
    let settings = driver.view()?.theme_settings();
    driver.target(chrome_target::launcher)?;
    driver.button(1, &[command::arm_pointer(chrome_target::launcher)])?;
    if driver.view()?.launcher_open() {
        return Err("launcher opened on press".into());
    }
    driver.motion(0, -64, &[command::arm_pointer(chrome_target::launcher)])?;
    driver.button(0, &[])?;
    if driver.view()?.launcher_open() {
        return Err("launcher opened after a drag-away release".into());
    }
    driver.target(chrome_target::launcher)?;
    driver.button(1, &[command::arm_pointer(chrome_target::launcher)])?;
    driver.motion(0, -64, &[command::arm_pointer(chrome_target::launcher)])?;
    driver.motion(0, 64, &[command::arm_pointer(chrome_target::launcher)])?;
    driver.button(0, &[command::toggle_launcher])?;
    let capture = driver.capture()?;
    if !driver.view()?.launcher_open()
        || driver.view()?.all_windows().count() != count
        || driver.view()?.focused_window().unwrap().id() != focused
    {
        return Err("launcher opening changed application ownership or focus".into());
    }
    driver.key("down", command::select_launcher(launcher_choice::inspector))?;
    driver.key("tab", command::select_launcher(launcher_choice::activity))?;
    driver.key(
        "shift-tab",
        command::select_launcher(launcher_choice::inspector),
    )?;
    for key in ["n", "c", "t", "f2", "f3"] {
        driver.ignored_key(key)?;
    }
    if driver.view()?.all_windows().count() != count || driver.view()?.theme_settings() != settings
    {
        return Err("launcher navigation leaked into application shortcuts".into());
    }
    driver.key("esc", command::dismiss_launcher)?;
    if driver.view()?.launcher_open() || driver.view()?.focused_window().unwrap().id() != focused {
        return Err("Escape failed to dismiss launcher without changing focus".into());
    }

    // Press outside on a real underlying Close control. Dismissal consumes
    // the complete gesture, so release must leave the application alive.
    let close = chrome_bounds(&driver.view()?, chrome_target::close(focused))
        .ok_or("launcher outside-click fixture has no Close control")?;
    driver.key("f1", command::toggle_launcher)?;
    driver.move_to(close.x + close.width / 2, close.y + close.height / 2)?;
    driver.button(1, &[command::dismiss_launcher])?;
    driver.button(0, &[])?;
    if driver.view()?.window(focused).is_none() || driver.view()?.launcher_open() {
        return Err("outside launcher dismissal clicked through to Close".into());
    }

    // A held menu item cannot survive close and reopen through the keyboard.
    driver.key("f1", command::toggle_launcher)?;
    let item = chrome_target::launcher_item(launcher_choice::activity);
    driver.target(item)?;
    driver.button(1, &[command::arm_pointer(item)])?;
    driver.key("esc", command::dismiss_launcher)?;
    driver.key("f1", command::toggle_launcher)?;
    driver.button(0, &[])?;
    if driver.view()?.all_windows().count() != count {
        return Err("stale launcher release created an application".into());
    }
    driver.click(chrome_target::launcher, &[command::toggle_launcher])?;
    Ok(capture)
}

fn resize_appearance(
    driver: &mut pointer_driver<'_>,
    window: window_id,
    width: u32,
    height: u32,
) -> Result<(), String> {
    driver.target(chrome_target::resize(window))?;
    driver.button(1, &[command::focus(window)])?;
    loop {
        let bounds = driver
            .view()?
            .window(window)
            .ok_or("Appearance disappeared during resize")?
            .floating_bounds();
        let dx = (i64::from(width) - i64::from(bounds.width())).clamp(-64, 64) as i32;
        let dy = (i64::from(height) - i64::from(bounds.height())).clamp(-64, 64) as i32;
        if dx == 0 && dy == 0 {
            break;
        }
        driver.motion(
            dx,
            dy,
            &[command::resize_window {
                window,
                width: (i64::from(bounds.width()) + i64::from(dx)) as u32,
                height: (i64::from(bounds.height()) + i64::from(dy)) as u32,
            }],
        )?;
    }
    driver.button(0, &[])
}

fn appearance_sequence(driver: &mut pointer_driver<'_>) -> Result<(Vec<u8>, Vec<u8>), String> {
    use appearance_choice::*;
    // Fill the two remaining slots using public key actions. Opening another
    // app at capacity must reject policy without retiring the native owner.
    if driver.view()?.all_windows().count() != 2 {
        return Err("Appearance capacity fixture requires the original two windows".into());
    }
    let mut temporary = Vec::new();
    for _ in 0..2 {
        driver.key(
            "n",
            command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds: window_rect::try_new(120, 140, 480, 320).map_err(|e| e.code())?,
            },
        )?;
        let window = driver
            .view()?
            .focused_window()
            .filter(|window| window.content() == demo_content::activity)
            .ok_or("capacity fixture did not focus its new Activity window")?
            .id();
        temporary.push(window);
    }
    let revision = driver.view()?.revision();
    let settings = driver.view()?.theme_settings();
    if driver.view()?.all_windows().count() != anomalous_desktop_workspace::window_capacity {
        return Err("Appearance capacity fixture did not fill all window slots".into());
    }
    match driver
        .model
        .prepare(&driver.human, revision, command::open_appearance)
    {
        Err(anomalous_desktop_workspace::desktop_error::window_capacity_exhausted) => {}
        _ => return Err("canonical Appearance open did not reject full capacity".into()),
    }
    if driver.view()?.revision() != revision
        || driver.view()?.theme_settings() != settings
        || driver.view()?.all_windows().count() != anomalous_desktop_workspace::window_capacity
    {
        return Err("rejected Appearance capacity action changed canonical state".into());
    }
    driver.ignored_key("p")?;
    driver.key("f1", command::toggle_launcher)?;
    if !driver.view()?.launcher_enabled(launcher_choice::welcome)
        || driver.view()?.launcher_enabled(launcher_choice::appearance)
    {
        return Err("full-capacity launcher availability is incorrect".into());
    }
    driver.key("down", command::select_launcher(launcher_choice::inspector))?;
    driver.key("down", command::select_launcher(launcher_choice::activity))?;
    driver.key(
        "down",
        command::select_launcher(launcher_choice::appearance),
    )?;
    driver.ignored_key("ret")?;
    driver.key("esc", command::dismiss_launcher)?;
    for window in temporary.into_iter().rev() {
        if driver.view()?.focused_window().map(|window| window.id()) != Some(window) {
            return Err("capacity fixture lost its expected Activity focus".into());
        }
        driver.key("q", command::close(window))?;
    }
    if driver.view()?.all_windows().count() != 2 {
        return Err("Appearance capacity fixture did not restore the original window count".into());
    }
    driver.click(
        chrome_target::appearance_launcher,
        &[command::open_appearance],
    )?;
    let window = appearance_window(driver)?;
    appearance_selection(driver, window, light)?;

    // The preceding desktop exercise deliberately leaves independent settings
    // changed. Reset through the actual application before retaining a fixture.
    appearance_click(driver, window, reset)?;
    if driver.view()?.theme_settings() != theme_settings::initial {
        return Err("Appearance reset failed to restore the complete initial profile".into());
    }
    appearance_click(driver, window, light)?;
    let light_capture = driver.capture()?;
    appearance_click(driver, window, dark)?;

    // Expected choices are written explicitly. The physical-key adapter and
    // its next/previous routing are not used to predict these transitions.
    driver.key(
        "f6",
        command::select_appearance {
            window,
            choice: high_contrast,
        },
    )?;
    appearance_selection(driver, window, high_contrast)?;
    if driver.view()?.theme() != theme_mode::dark {
        return Err("Appearance selection applied a setting before activation".into());
    }
    driver.key(
        "shift-f6",
        command::select_appearance {
            window,
            choice: dark,
        },
    )?;
    driver.key(
        "f6",
        command::select_appearance {
            window,
            choice: high_contrast,
        },
    )?;
    driver.key(
        "ret",
        command::activate_appearance {
            window,
            choice: high_contrast,
        },
    )?;
    driver.key(
        "shift-f6",
        command::select_appearance {
            window,
            choice: dark,
        },
    )?;
    driver.key(
        "spc",
        command::activate_appearance {
            window,
            choice: dark,
        },
    )?;
    let dark_capture = driver.capture()?;

    // Every component has explicit values, including both directions of text
    // geometry, cursor shape, corners, depth, and the stored motion preference.
    for choice in [
        compact,
        comfortable,
        large_text,
        regular_text,
        crosshair_cursor,
        arrow_cursor,
        square_frames,
        rounded_frames,
        depth_off,
        depth_on,
        reduced_motion_off,
        reduced_motion_on,
    ] {
        appearance_click(driver, window, choice)?;
        if !driver.view()?.appearance_selected(choice) {
            return Err("Appearance option did not select its explicit preference".into());
        }
    }

    driver.click(
        chrome_target::minimize(window),
        &[command::minimize(window)],
    )?;
    driver.key("p", command::open_appearance)?;
    if appearance_window(driver)? != window {
        return Err("keyboard Appearance reopen replaced its minimized window".into());
    }
    appearance_selection(driver, window, reduced_motion_on)?;

    driver.key(
        "w",
        command::move_to_workspace {
            window,
            workspace: workspace_id::second,
        },
    )?;
    driver.click(
        chrome_target::appearance_launcher,
        &[command::open_appearance],
    )?;
    if appearance_window(driver)? != window
        || driver.view()?.window(window).unwrap().workspace() != workspace_id::first
    {
        return Err("Appearance header reopen did not reuse its remote-workspace window".into());
    }
    driver.key("2", command::switch_workspace(workspace_id::second))?;
    driver.key("p", command::open_appearance)?;
    if appearance_window(driver)? != window
        || driver.view()?.window(window).unwrap().workspace() != workspace_id::second
    {
        return Err("Appearance keyboard reopen did not reuse its remote-workspace window".into());
    }
    driver.key("1", command::switch_workspace(workspace_id::first))?;
    driver.click(
        chrome_target::appearance_launcher,
        &[command::open_appearance],
    )?;

    let original = driver.view()?.window(window).unwrap().floating_bounds();
    resize_appearance(driver, window, 320, 180)?;
    let view = driver.view()?;
    if appearance_controls_visible(&view, window)
        || appearance_choice::all.into_iter().any(|choice| {
            chrome_bounds(&view, chrome_target::appearance_choice(window, choice)).is_some()
        })
    {
        return Err("small Appearance fallback exposes invisible preference targets".into());
    }
    driver.ignored_key("f6")?;
    driver.ignored_key("ret")?;
    driver.ignored_key("spc")?;
    resize_appearance(driver, window, original.width(), original.height())?;
    if !appearance_controls_visible(&driver.view()?, window) {
        return Err("enlarged Appearance window did not restore visible controls".into());
    }
    appearance_click(driver, window, reset)?;
    if driver.view()?.theme_settings() != theme_settings::initial {
        return Err("Appearance reset left a component override behind".into());
    }
    Ok((light_capture, dark_capture))
}

fn expect_armed(
    driver: &pointer_driver<'_>,
    expected: Option<chrome_target>,
) -> Result<(), String> {
    if driver.view()?.pointer_armed() != expected {
        return Err("canonical pointer arming differs from the physical gesture".into());
    }
    Ok(())
}

fn create_release_fixture(driver: &mut pointer_driver<'_>) -> Result<window_id, String> {
    driver.key(
        "n",
        command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds: window_rect::try_new(120, 140, 480, 320).map_err(|e| e.code())?,
        },
    )?;
    driver
        .view()?
        .focused_window()
        .filter(|window| window.content() == demo_content::activity)
        .map(|window| window.id())
        .ok_or("release fixture did not focus its new Activity window".into())
}

/// Expected actions are deliberate model commands, independent of the native
/// gesture adapter. Each packet checks a complete frame before the next input.
fn release_sequence(driver: &mut pointer_driver<'_>) -> Result<(), String> {
    let appearance = appearance_window(driver)?;
    let original_count = driver.view()?.all_windows().count();
    let window = create_release_fixture(driver)?;
    let close = chrome_target::close(window);
    driver.target(close)?;
    driver.button(1, &[command::arm_pointer(close)])?;
    expect_armed(driver, Some(close))?;
    if driver.view()?.window(window).is_none() {
        return Err("Close destroyed its window on pointer press".into());
    }
    // Leave the original control while held. The semantic arm survives for
    // possible reentry, but release over the client must cancel the action.
    driver.motion(-40, 40, &[command::arm_pointer(close)])?;
    expect_armed(driver, Some(close))?;
    driver.button(0, &[])?;
    expect_armed(driver, None)?;
    if driver.view()?.window(window).is_none() {
        return Err("Close destroyed its window after a drag-away release".into());
    }

    // Returning to the same topmost target permits exactly one activation.
    driver.target(close)?;
    driver.button(1, &[command::arm_pointer(close)])?;
    driver.motion(-40, 40, &[command::arm_pointer(close)])?;
    driver.motion(40, -40, &[command::arm_pointer(close)])?;
    expect_armed(driver, Some(close))?;
    driver.button(0, &[command::close(window)])?;
    expect_armed(driver, None)?;
    if driver.view()?.window(window).is_some()
        || driver.view()?.all_windows().count() != original_count
    {
        return Err("Close did not act exactly once on a matching release".into());
    }

    let dark = chrome_target::appearance_choice(appearance, appearance_choice::dark);
    let light = chrome_target::appearance_choice(appearance, appearance_choice::light);
    // Start on the exposed right desktop margin, then pass over a control.
    // A held button without an initial control press must not arm or depress it.
    driver.move_to(1020, 700)?;
    driver.button(1, &[])?;
    driver.target(dark)?;
    expect_armed(driver, None)?;
    driver.button(0, &[])?;
    if driver.view()?.theme_settings() != theme_settings::initial {
        return Err("held-from-desktop motion activated an Appearance choice".into());
    }

    driver.button(1, &[command::arm_pointer(dark)])?;
    expect_armed(driver, Some(dark))?;
    if driver.view()?.theme_settings() != theme_settings::initial {
        return Err("Appearance changed its setting on pointer press".into());
    }
    driver.button(
        0,
        &[command::activate_appearance {
            window: appearance,
            choice: appearance_choice::dark,
        }],
    )?;
    if driver.view()?.theme() != theme_mode::dark {
        return Err("Appearance failed to apply its matching release".into());
    }

    // A mapped keyboard command supersedes the gesture in its own transaction.
    driver.target(light)?;
    driver.button(1, &[command::arm_pointer(light)])?;
    driver.key("d", command::set_density(density_mode::compact))?;
    expect_armed(driver, None)?;
    driver.button(0, &[])?;
    if driver.view()?.theme() != theme_mode::dark
        || driver.view()?.density() != density_mode::compact
    {
        return Err("mapped keyboard cancellation allowed a stale release action".into());
    }

    // Unmapped B still cancels an existing arm with one canonical frame. Its
    // break event has no second mutation; the exact transcript verifies that.
    driver.target(light)?;
    driver.button(1, &[command::arm_pointer(light)])?;
    driver.key("b", command::cancel_pointer)?;
    expect_armed(driver, None)?;
    driver.button(0, &[])?;
    if driver.view()?.theme() != theme_mode::dark {
        return Err("unmapped keyboard cancellation allowed a stale release action".into());
    }

    // Adding another button cancels the arm. Returning to left-only cannot
    // reacquire it; all buttons must be released before a fresh press.
    driver.button(1, &[command::arm_pointer(light)])?;
    driver.button(3, &[])?;
    expect_armed(driver, None)?;
    driver.button(1, &[])?;
    expect_armed(driver, None)?;
    driver.button(0, &[])?;
    if driver.view()?.theme() != theme_mode::dark {
        return Err("button chord released a canceled Appearance action".into());
    }
    appearance_click(driver, appearance, appearance_choice::light)?;

    // Reusing a window slot while held must not make an old Close gesture
    // delete the replacement generation at the identical screen position.
    let retired = create_release_fixture(driver)?;
    driver.target(chrome_target::close(retired))?;
    driver.button(1, &[command::arm_pointer(chrome_target::close(retired))])?;
    driver.key("q", command::close(retired))?;
    expect_armed(driver, None)?;
    let replacement = create_release_fixture(driver)?;
    if replacement == retired || replacement.slot() != retired.slot() {
        return Err("release fixture did not reuse a fresh window-slot generation".into());
    }
    driver.button(0, &[])?;
    if driver.view()?.window(replacement).is_none() {
        return Err("stale Close release deleted a replacement window generation".into());
    }
    driver.click(
        chrome_target::close(replacement),
        &[command::close(replacement)],
    )?;

    appearance_click(driver, appearance, appearance_choice::reset)?;
    expect_armed(driver, None)?;
    if driver.view()?.theme_settings() != theme_settings::initial
        || driver.view()?.all_windows().count() != original_count
        || appearance_window(driver)? != appearance
    {
        return Err("release exercise changed its restored Appearance singleton fixture".into());
    }
    Ok(())
}

pub(super) fn run(
    root: &Path,
    image: &Path,
    pages: u64,
    profile: workspace_profile,
) -> Result<usize, String> {
    let directory = profile.directory(root, "workspace-pointer-smoke");
    fs::create_dir_all(&directory).map_err(|_| "create pointer smoke artifact directory")?;
    let mut files = captures::new_profile(&directory, profile)?;
    let child = Command::new("qemu-system-x86_64")
        .args(profile.cpu_arguments())
        .current_dir(root)
        .args(["-machine", "q35", "-m", "256M", "-drive"])
        .arg(format!("format=raw,file={}", image.display()))
        .args(["-no-reboot", "-no-shutdown", "-display", "none", "-serial"])
        .arg(format!("file:{}", files.log_part.display()))
        .args(["-monitor", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "launch native pointer smoke QEMU")?;
    let mut child = semantic_presentation_child::new(child);
    let result = (|| {
        let mut driver = pointer_driver::new(child.child_mut(), &files, pages)?;
        let window = first_window(&driver)?;
        // Select the visible first title; focus raises it above the inspector.
        driver.target(chrome_target::titlebar(window))?;
        driver.button(1, &[command::focus(window)])?;
        let bounds = driver.view()?.window(window).unwrap().floating_bounds();
        driver.motion(
            24,
            16,
            &[command::move_window {
                window,
                x: bounds.x() + 24,
                y: bounds.y() + 16,
            }],
        )?;
        driver.button(0, &[])?;
        driver.target(chrome_target::resize(window))?;
        driver.button(1, &[command::focus(window)])?;
        let bounds = driver.view()?.window(window).unwrap().floating_bounds();
        driver.motion(
            24,
            16,
            &[command::resize_window {
                window,
                width: bounds.width() + 24,
                height: bounds.height() + 16,
            }],
        )?;
        driver.button(0, &[])?;
        driver.click(
            chrome_target::maximize(window),
            &[command::maximize(window)],
        )?;
        driver.click(
            chrome_target::maximize(window),
            &[command::restore_geometry(window), command::focus(window)],
        )?;
        driver.click(
            chrome_target::minimize(window),
            &[command::minimize(window)],
        )?;
        driver.click(
            chrome_target::task(window),
            &[command::restore(window), command::focus(window)],
        )?;
        let light = driver.capture()?;
        let launcher_light = launcher_sequence(&mut driver)?;
        driver.click(chrome_target::launcher, &[command::toggle_launcher])?;
        launcher_click(&mut driver, launcher_choice::appearance)?;
        let appearance = appearance_window(&driver)?;
        appearance_click(&mut driver, appearance, appearance_choice::dark)?;
        let dark = driver.capture()?;
        driver.key("f1", command::toggle_launcher)?;
        let launcher_dark = driver.capture()?;
        driver.key("esc", command::dismiss_launcher)?;
        appearance_click(&mut driver, appearance, appearance_choice::compact)?;
        appearance_click(&mut driver, appearance, appearance_choice::large_text)?;
        appearance_click(&mut driver, appearance, appearance_choice::crosshair_cursor)?;
        driver.key("q", command::close(appearance))?;
        driver.key("t", command::set_layout(layout_mode::horizontal))?;
        driver.key("t", command::set_layout(layout_mode::vertical))?;
        driver.key("t", command::set_layout(layout_mode::floating))?;
        driver.click(
            chrome_target::workspace(workspace_id::second),
            &[command::switch_workspace(workspace_id::second)],
        )?;
        driver.click(chrome_target::launcher, &[command::toggle_launcher])?;
        launcher_click(&mut driver, launcher_choice::activity)?;
        let created = driver
            .view()?
            .focused_window()
            .ok_or("launcher did not create public activity")?
            .id();
        driver.click(chrome_target::close(created), &[command::close(created)])?;
        driver.click(
            chrome_target::workspace(workspace_id::first),
            &[command::switch_workspace(workspace_id::first)],
        )?;
        let appearance_start = driver.frames;
        let (appearance_light, appearance_dark) = appearance_sequence(&mut driver)
            .map_err(|error| format!("Appearance smoke: {error}"))?;
        let mut appearance_frames = driver.frames - appearance_start;
        let release_start = driver.frames;
        release_sequence(&mut driver).map_err(|error| format!("Release gesture smoke: {error}"))?;
        let release_frames = driver.frames - release_start;
        let first_frames = driver.frames;
        driver.withdraw()?;
        driver.restart()?;
        // The second session must accept source generation two and have exact
        // initial model, hidden cursor, and entirely reclaimed prior resources.
        if driver.view()?.launcher_open()
            || driver.view()?.launcher_selection() != launcher_choice::welcome
        {
            return Err("fresh desktop retained a launcher session".into());
        }
        driver.motion(16, 8, &[])?;
        let fresh_start = driver.frames;
        driver.key("p", command::open_appearance)?;
        let appearance = appearance_window(&driver)?;
        appearance_selection(&driver, appearance, appearance_choice::light)?;
        if driver.view()?.theme_settings() != theme_settings::initial {
            return Err("fresh Appearance session inherited a retired preference".into());
        }
        appearance_frames += driver.frames - fresh_start;
        driver.withdraw()?;
        let frames = first_frames + driver.frames;
        Ok::<_, String>((
            light,
            dark,
            launcher_light,
            launcher_dark,
            appearance_light,
            appearance_dark,
            driver.records,
            frames,
            appearance_frames,
            release_frames,
        ))
    })();
    child.terminate_and_reap();
    let (
        light,
        dark,
        launcher_light,
        launcher_dark,
        appearance_light,
        appearance_dark,
        records,
        frames,
        appearance_frames,
        release_frames,
    ) = result?;
    let transcript = read_bounded_optional_utf8_file(&files.log_part, transcript_limit)?;
    verify_final_records(profile, &transcript, &records)?;
    for (path, data) in [
        (&files.log, transcript.as_bytes()),
        (&files.initial, light.as_slice()),
        (&files.customized, dark.as_slice()),
        (&files.launcher_light, launcher_light.as_slice()),
        (&files.launcher_dark, launcher_dark.as_slice()),
        (&files.appearance_light, appearance_light.as_slice()),
        (&files.appearance_dark, appearance_dark.as_slice()),
    ] {
        create_private_capture_file(path)?;
        fs::write(path, data).map_err(|_| "retain verified pointer fixture artifact")?;
    }
    files.published = true;
    println!(
        "Appearance native exercise added {appearance_frames} exact frames; {} existing pointer frames retained",
        frames - appearance_frames - release_frames
    );
    println!(
        "Release gesture native exercise added {release_frames} exact frames; settings and window controls activate only on an armed matching release"
    );
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_cpu_pointer_lifetimes_require_validated_evidence_before_exact_comparison() {
        let cpu_ready = r#"{"schema":1,"event":"owned_workspace_cpu.ready","level":"info","scenario":"desktop","task_images":1,"image_bytes":512,"observation_bytes":512,"cpu_vendor":"GenuineIntel","initial_clean":true}"#;
        let cpu_release = r#"{"schema":1,"event":"owned_workspace_cpu.released","level":"info","scenario":"desktop","gates":773,"timers":4,"inputs":0,"other_irqs":0,"faults":0,"probes":4,"clean_checks":777,"user_checks":777,"x87_timers":2,"mmx_timers":2,"probe_complete":true,"controls_restored":true,"storage_cleared":true}"#;
        let mut records = Vec::new();
        let mut raw = Vec::new();
        for generation in 1..=2 {
            let prepared = owned_workspace::prepared_record("desktop");
            records.push(prepared.clone());
            raw.extend([prepared, cpu_ready.to_owned()]);
            for record in [
                pointer_record(generation),
                ready_record(32),
                frame_record(1),
            ] {
                records.push(record.clone());
                raw.push(record);
            }
            let released = owned_workspace::released_record("desktop", "task_exit", 1);
            records.push(released.clone());
            raw.extend([cpu_release.to_owned(), released]);
            for record in fallback_records(1, "task_exit") {
                records.push(record.clone());
                raw.push(record);
            }
        }
        let transcript = raw.join("\r\n") + "\r\n";
        // This exact raw comparison was the final-publication regression:
        // the incremental reader had already validated/normalized CPU events.
        assert!(!exact_records(&transcript, &records));
        verify_final_records(workspace_profile::owned_cpu, &transcript, &records).unwrap();
        for profile in [workspace_profile::legacy, workspace_profile::owned] {
            assert!(verify_final_records(profile, &transcript, &records).is_err());
            verify_final_records(profile, &(records.join("\r\n") + "\r\n"), &records).unwrap();
        }
        for changed in [
            transcript.replace("\"controls_restored\":true", "\"controls_restored\":false"),
            transcript.replace("\"user_checks\":777", "\"user_checks\":776"),
            transcript.replace("\"generation\":2", "\"generation\":1"),
            transcript.replace(&(cpu_release.to_owned() + "\r\n"), ""),
            transcript.clone() + "private extra record\r\n",
            transcript[..transcript.len() - 1].to_owned(),
        ] {
            assert!(
                verify_final_records(workspace_profile::owned_cpu, &changed, &records).is_err()
            );
        }
        let incomplete = owned_workspace::prepared_record("desktop") + "\r\n";
        assert!(verify_final_records(workspace_profile::owned_cpu, &incomplete, &records).is_err());
        // Verification borrows the original transcript; publication retains
        // its complete validated CPU evidence, not the normalized comparison.
        assert_eq!(transcript.matches(cpu_ready).count(), 2);
        assert_eq!(transcript.matches(cpu_release).count(), 2);
    }

    #[test]
    fn appearance_captures_are_removed_when_publication_fails() {
        let directory = std::env::temp_dir().join(format!(
            "anomalous-appearance-capture-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let paths;
        {
            let files = captures::new(&directory).unwrap();
            paths = [
                files.log_part.clone(),
                files.frame_part.clone(),
                files.log.clone(),
                files.initial.clone(),
                files.customized.clone(),
                files.launcher_light.clone(),
                files.launcher_dark.clone(),
                files.appearance_light.clone(),
                files.appearance_dark.clone(),
            ];
            for path in &paths {
                fs::write(path, b"unverified-private-sentinel").unwrap();
            }
        }
        for path in paths {
            assert!(!path.exists());
        }
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn repeat_lifecycle_rejects_old_generation_extra_content_and_missing_withdrawal() {
        let mut records = vec![pointer_record(1), ready_record(12), frame_record(1)];
        records.extend(fallback_records(1, "task_exit"));
        records.extend(restart_records(12, 2));
        records.push(frame_record(2));
        records.extend(fallback_records(2, "task_exit"));
        let text = records.join("\r\n") + "\r\n";
        assert!(exact_records(&text, &records));
        assert!(!exact_records(
            &text.replace("\"generation\":2", "\"generation\":1"),
            &records
        ));
        assert!(!exact_records(
            &text.replace(
                "\"resources_restored\":true",
                "\"resources_restored\":false"
            ),
            &records
        ));
        assert!(!exact_records(
            &(text.clone() + "private pointer data\r\n"),
            &records
        ));
        for index in 0..records.len() {
            let mut removed = records.clone();
            removed.remove(index);
            assert!(!exact_records(&(removed.join("\r\n") + "\r\n"), &records));
        }
    }
}
