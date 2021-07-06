#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate gl;
extern crate glutin;

use std::f32::consts::{FRAC_PI_2, PI};
use std::ffi::CStr;
use std::ffi::CString;
use std::io::Write;
use std::time::SystemTime;
use std::{fs, mem, ptr, str};

use serde::{Deserialize, Serialize};

use gl::types::*;
use glutin::dpi::{PhysicalPosition, PhysicalSize};
use glutin::event::{
    ElementState, Event, MouseButton, MouseScrollDelta, TouchPhase, VirtualKeyCode, WindowEvent,
};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::monitor::MonitorHandle;
use glutin::window::Window;
use glutin::ContextWrapper;

// Shader sources
static VS_SRC: &'static str = include_str!("shader.vert");
static FS_SRC: &'static str = include_str!("shader.frag");

const N_CURSOR_RETICLE_POINTS: usize = 32;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    config_version: u8,
    smoothing_range: usize,
    smoothing_intensity: usize,
    default_brush_size: f32,
    default_brush_color_index: i32,
    brush_colors: [[u32; 3]; 8],
    brush_sizes: [f32; 5],
    background_color: [u32; 3],
    background_color_opacity: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: 1,
            smoothing_range: 1,
            smoothing_intensity: 1,
            default_brush_size: 3.0,
            default_brush_color_index: 0,
            brush_colors: [
                [255, 255, 255], // white
                [10, 10, 10],    // black
                [255, 150, 0],   // orange
                [255, 0, 220],   // pink
                [255, 50, 50],   // red
                [25, 255, 75],   // green
                [25, 75, 255],   // blue
                [255, 255, 0],   // yellow
            ],
            brush_sizes: [1.0, 3.0, 5.0, 10.0, 30.0],
            background_color: [0, 0, 0],
            background_color_opacity: 0.8,
        }
    }
}

#[derive(Default)]
struct Input {
    modifiers: Modifiers,
    cursor: Cursor,
}

#[derive(Default)]
struct Modifiers {
    shift: bool,
    ctrl: bool,
    alt: bool,
    logo: bool,
}

#[derive(Default, Debug)]
struct Cursor {
    x: f32,
    y: f32,
    last_x: f32,
    last_y: f32,
    pressed: bool,
    released_time: Option<SystemTime>,
}

#[derive(Default, Debug)]
struct Rect2D {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Default, Debug)]
struct Size2D {
    width: f32,
    height: f32,
}

#[derive(Default, Debug, Copy, Clone)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

impl Point {
    fn into_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Default)]
struct LineStyle {
    color: [f32; 3],
    width: f32,
    pressure: f32,
    smoothing_range: usize,
    smoothing_intensity: usize,
}

struct GLState {
    window_context: ContextWrapper<glutin::PossiblyCurrent, Window>,
    program: u32,
    fs: u32,
    vs: u32,
    vao: u32,
    vbo: u32,
}

struct DrawingState {
    config: Config,
    need_redraw: bool,
    is_window_hidden: bool,
    is_background_visible: bool,
    n_points_current_line: u32,
    line_style: LineStyle,
    gl_context: GLState,
    undo_steps: Vec<usize>,
    smooth_index: usize,
    vertices: Vec<f32>,
    rect: Rect2D,
}

fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    let shader;
    unsafe {
        shader = gl::CreateShader(ty);
        // Attempt to compile the shader
        let c_str = CString::new(src.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        // Get the compile status
        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetShaderInfoLog(
                shader,
                len,
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );
            panic!(
                "{}",
                str::from_utf8(&buf)
                    .ok()
                    .expect("ShaderInfoLog not valid utf8")
            );
        }
    }
    shader
}

fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    unsafe {
        let program = gl::CreateProgram();
        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);
        gl::LinkProgram(program);
        // Get the link status
        let mut status = gl::FALSE as GLint;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len: GLint = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetProgramInfoLog(
                program,
                len,
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );
            panic!(
                "{}",
                str::from_utf8(&buf)
                    .ok()
                    .expect("ProgramInfoLog not valid utf8")
            );
        }
        program
    }
}

fn screen_size_to_gl(w: f32, h: f32, overlay_rect: &Rect2D) -> Size2D {
    Size2D {
        width: w / overlay_rect.width * 2.0,
        height: h / overlay_rect.height * 2.0,
    }
}

#[allow(dead_code)]
fn screen_point_to_gl(x: f32, y: f32, overlay_rect: &Rect2D) -> Point {
    Point {
        x: x / overlay_rect.width * 2.0,
        y: y / overlay_rect.height * 2.0,
        z: 0.0,
    }
}

fn init_gl_window(event_loop: &EventLoop<()>, overlay_rect: &Rect2D) -> GLState {
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Inke")
        .with_inner_size(PhysicalSize::new(overlay_rect.width, overlay_rect.height))
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_visible(false);

    let gl_window = glutin::ContextBuilder::new()
        .with_multisampling(8)
        .build_windowed(window_builder, &event_loop)
        .unwrap();

    let gl_window = unsafe { gl_window.make_current() }.unwrap();

    gl_window
        .window()
        .set_outer_position(PhysicalPosition::new(overlay_rect.x, overlay_rect.y));
    gl_window.window().set_visible(true);

    // Load the OpenGL function pointers
    gl::load_with(|symbol| gl_window.get_proc_address(symbol));

    let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
    let fs = compile_shader(FS_SRC, gl::FRAGMENT_SHADER);
    let program = link_program(vs, fs);

    let mut vao = 0;
    let mut vbo = 0;

    unsafe {
        // Create Vertex Array Object
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        // Create Vertex Buffer Object
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

        // Use shader program
        gl::UseProgram(program);
        gl::BindFragDataLocation(
            program,
            0,
            CStr::from_bytes_with_nul(b"out_color\0").unwrap().as_ptr(),
        );

        // position attrib
        let pos_attr = gl::GetAttribLocation(
            program,
            CStr::from_bytes_with_nul(b"position\0").unwrap().as_ptr(),
        );
        gl::EnableVertexAttribArray(pos_attr as GLuint);
        gl::VertexAttribPointer(
            pos_attr as GLuint,                                   // index of attribute
            3,                                                    // the number of components
            gl::FLOAT,                                            // data type
            gl::FALSE as GLboolean,                               // normalized
            (6 * std::mem::size_of::<f32>()) as gl::types::GLint, // stride (byte offset)
            ptr::null(),                                          // offset of the first component
        );

        // vertex_color attrib
        let color_attr = gl::GetAttribLocation(
            program,
            CStr::from_bytes_with_nul(b"vColor\0").unwrap().as_ptr(),
        );
        gl::EnableVertexAttribArray(color_attr as GLuint);
        gl::VertexAttribPointer(
            color_attr as GLuint,                                 // index of attribute
            3,                                                    // the number of components
            gl::FLOAT,                                            // data type
            gl::FALSE as GLboolean,                               // normalized
            (6 * std::mem::size_of::<f32>()) as gl::types::GLint, // stride (byte offset)
            (3 * std::mem::size_of::<f32>()) as *const gl::types::GLvoid, // offset of the first component
        );
    };

    GLState {
        window_context: gl_window,
        program: program,
        vs: vs,
        fs: fs,
        vbo: vbo,
        vao: vao,
    }
}

/// Apply line smoothing to parts of a point list
///
/// Reference: https://stackoverflow.com/a/18830268
fn apply_line_smoothing(points: &mut [f32], smoothing_range: usize) {
    if smoothing_range == 0 {
        return;
    }

    // Number of line endings to parse
    let line_segment_len = 3 * 2 * 6; // 3 (points per triangle) * 2 (triangle) * 6 (properties x,y,z,r,g,b)
    let n_points = (points.len() / line_segment_len) - 1; // -1 to skip last

    // skip first
    for i in 1..n_points {
        let start = if i >= smoothing_range {
            i - smoothing_range
        } else {
            0
        };
        let end = if i + smoothing_range < n_points {
            i + smoothing_range
        } else {
            n_points
        };

        // sums for left side of line (p3, p1)
        let mut sum_x1 = 0.0_f32;
        let mut sum_y1 = 0.0_f32;
        // sums for right side of line (p4, p2)
        let mut sum_x2 = 0.0_f32;
        let mut sum_y2 = 0.0_f32;

        for j in start..end {
            // x,y of p3 for that line segment
            sum_x1 += points[j * line_segment_len];
            sum_y1 += points[j * line_segment_len + 1];
            sum_x2 += points[j * line_segment_len + 30];
            sum_y2 += points[j * line_segment_len + 30 + 1];
        }

        let avg_x1 = sum_x1 / ((end - start) as f32);
        let avg_y1 = sum_y1 / ((end - start) as f32);
        let avg_x2 = sum_x2 / ((end - start) as f32);
        let avg_y2 = sum_y2 / ((end - start) as f32);

        /*
          0  18__ 30
           |\­  \ |
           | \  \|
          12¯¯6   24

         36  54__ 66
           |\­  \ |
           | \  \|
          48¯¯42  60
        */

        // LEFT SIDE
        // p3 - 24 = p1 of last segment
        points[i * line_segment_len - 24] = avg_x1;
        points[i * line_segment_len - 24 + 1] = avg_y1;

        // p3
        points[i * line_segment_len] = avg_x1;
        points[i * line_segment_len + 1] = avg_y1;

        // p3 (second triange)
        points[i * line_segment_len + 18] = avg_x1;
        points[i * line_segment_len + 18 + 1] = avg_y1;

        // RIGHT SIDE
        // p4 - 60 = p2 of last segment (first triangle)
        points[i * line_segment_len + 30 - 60] = avg_x2;
        points[i * line_segment_len + 30 - 60 + 1] = avg_y2;
        // p4 - 42 = p2 of last segment (second triangle)
        points[i * line_segment_len + 30 - 42] = avg_x2;
        points[i * line_segment_len + 30 - 42 + 1] = avg_y2;
        // p4
        points[i * line_segment_len + 30] = avg_x2;
        points[i * line_segment_len + 30 + 1] = avg_y2;
    }
}

fn get_overlay_rect(monitors: impl Iterator<Item = MonitorHandle>) -> Rect2D {
    let mut min_x: i32 = 0;
    let mut min_y: i32 = 0;
    let mut max_x: i32 = 0;
    let mut max_y: i32 = 0;

    for monitor in monitors {
        // println!(
        //     "Monitor {} = x: {}, y: {}, w: {}, h: {}",
        //     monitor.name().unwrap(),
        //     monitor.position().x,
        //     monitor.position().y,
        //     monitor.size().width,
        //     monitor.size().height
        // );
        if monitor.position().x < min_x {
            min_x = monitor.position().x;
        }
        if monitor.position().y < min_y {
            min_y = monitor.position().y;
        }
        if monitor.position().x + (monitor.size().width as i32) > max_x {
            max_x = monitor.position().x + (monitor.size().width as i32);
        }
        if monitor.position().y + (monitor.size().height as i32) > max_y {
            max_y = monitor.position().y + (monitor.size().height as i32);
        }
    }

    Rect2D {
        x: min_x as f32,
        y: min_y as f32,
        width: (max_x - min_x) as f32,
        height: (max_y - min_y) as f32,
    }
}

fn handle_event(
    event: Event<()>,
    control_flow: &mut ControlFlow,
    drawing: &mut DrawingState,
    input: &mut Input,
) -> () {
    *control_flow = ControlFlow::Wait;

    match event {
        Event::LoopDestroyed => return,
        Event::WindowEvent { event, .. } => match event {
            // Alt-tab in and out
            WindowEvent::Focused(has_focus) => {
                if has_focus {
                    // unhide
                    drawing.is_window_hidden = false;
                } else {
                    // force window to minimize
                    drawing
                        .gl_context
                        .window_context
                        .window()
                        .set_minimized(true);
                }
            }
            WindowEvent::ModifiersChanged(modifier) => {
                input.modifiers.logo = modifier.logo();
                input.modifiers.alt = modifier.alt();
                input.modifiers.shift = modifier.shift();
                input.modifiers.ctrl = modifier.ctrl();
            }
            WindowEvent::KeyboardInput {
                device_id: _,
                input: keyboard_input,
                is_synthetic: _,
            } => {
                if keyboard_input.state == glutin::event::ElementState::Released {
                    match keyboard_input.virtual_keycode {
                        None => (),
                        Some(key) => {
                            match key {
                                // escape
                                VirtualKeyCode::Escape => {
                                    // Todo: Request close event
                                    unsafe {
                                        gl::DeleteProgram(drawing.gl_context.program);
                                        gl::DeleteShader(drawing.gl_context.fs);
                                        gl::DeleteShader(drawing.gl_context.vs);
                                        gl::DeleteBuffers(1, &drawing.gl_context.vbo);
                                        gl::DeleteVertexArrays(1, &drawing.gl_context.vao);
                                    }
                                    *control_flow = ControlFlow::Exit
                                }
                                VirtualKeyCode::H => {
                                    drawing.need_redraw = true;
                                    // TODO: Show help
                                }
                                VirtualKeyCode::B => {
                                    // Toggle background
                                    drawing.need_redraw = true;
                                    drawing.is_background_visible = !drawing.is_background_visible;
                                }
                                VirtualKeyCode::Space => {
                                    // Clear drawings
                                    drawing.need_redraw = true;
                                    drawing.vertices.clear(); //resize(0, 0.0);
                                    drawing.undo_steps.clear();
                                    drawing.n_points_current_line = 0;
                                    drawing.smooth_index = 0;
                                }
                                VirtualKeyCode::Z => {
                                    // ctrl-z or cmd-z
                                    if input.modifiers.ctrl || input.modifiers.logo {
                                        // Undo (if any undo steps are available)
                                        match drawing.undo_steps.pop() {
                                            Some(n) => {
                                                drawing.vertices.resize(n, 0.0);
                                                drawing.need_redraw = true;
                                                drawing.n_points_current_line = 0;
                                                drawing.smooth_index = drawing.vertices.len();
                                            }
                                            None => (),
                                        }
                                    }
                                }

                                // q,w,e,r,... for line colors

                                // q (white)
                                VirtualKeyCode::Q => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[0]);
                                    drawing.need_redraw = true;
                                }
                                // w (black)
                                VirtualKeyCode::W => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[1]);
                                    drawing.need_redraw = true;
                                }
                                // e (orange)
                                VirtualKeyCode::E => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[2]);
                                    drawing.need_redraw = true;
                                }
                                // r (pink)
                                VirtualKeyCode::R => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[3]);
                                    drawing.need_redraw = true;
                                }
                                // t (red)
                                VirtualKeyCode::T => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[4]);
                                    drawing.need_redraw = true;
                                }
                                // y (green)
                                VirtualKeyCode::Y => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[5]);
                                    drawing.need_redraw = true;
                                }
                                // u (blue)
                                VirtualKeyCode::U => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[6]);
                                    drawing.need_redraw = true;
                                }
                                // i (yellow)
                                VirtualKeyCode::I => {
                                    drawing.line_style.color =
                                        color_to_gl(drawing.config.brush_colors[7]);
                                    drawing.need_redraw = true;
                                }

                                // 1,2,3,... for size
                                VirtualKeyCode::Key1 => {
                                    drawing.line_style.width = drawing.config.brush_sizes[0];
                                    drawing.need_redraw = true;
                                }
                                VirtualKeyCode::Key2 => {
                                    drawing.line_style.width = drawing.config.brush_sizes[1];
                                    drawing.need_redraw = true;
                                }
                                VirtualKeyCode::Key3 => {
                                    drawing.line_style.width = drawing.config.brush_sizes[2];
                                    drawing.need_redraw = true;
                                }
                                VirtualKeyCode::Key4 => {
                                    drawing.line_style.width = drawing.config.brush_sizes[3];
                                    drawing.need_redraw = true;
                                }
                                VirtualKeyCode::Key5 => {
                                    drawing.line_style.width = drawing.config.brush_sizes[4];
                                    drawing.need_redraw = true;
                                }

                                _ => (),
                            }
                        }
                    }
                }
            }
            WindowEvent::Touch(touch_event) => {
                drawing.need_redraw = true;

                if touch_event.phase == TouchPhase::Started {
                    input.cursor.pressed = true;
                }
                if touch_event.phase == TouchPhase::Ended
                    || touch_event.phase == TouchPhase::Cancelled
                {
                    input.cursor.pressed = false;
                    input.cursor.released_time = Some(SystemTime::now());

                    for _ in 0..drawing.line_style.smoothing_intensity {
                        apply_line_smoothing(
                            &mut drawing.vertices[drawing.smooth_index..],
                            drawing.line_style.smoothing_range,
                        );
                    }
                    drawing.smooth_index = drawing.vertices.len();

                    drawing.need_redraw = true;
                }

                input.cursor.last_x = input.cursor.x;
                input.cursor.last_y = input.cursor.y;
                input.cursor.x = touch_event.location.x as f32;
                input.cursor.y = touch_event.location.y as f32;

                match touch_event.force {
                    Some(force_type) => match force_type {
                        glutin::event::Force::Calibrated {
                            force,
                            max_possible_force,
                            altitude_angle: _,
                        } => {
                            drawing.line_style.pressure = (force / max_possible_force) as f32;
                        }
                        glutin::event::Force::Normalized(force) => {
                            drawing.line_style.pressure = force as f32;
                        }
                    },
                    None => (),
                }
            }
            WindowEvent::CloseRequested => {
                unsafe {
                    gl::DeleteProgram(drawing.gl_context.program);
                    gl::DeleteShader(drawing.gl_context.fs);
                    gl::DeleteShader(drawing.gl_context.vs);
                    gl::DeleteBuffers(1, &drawing.gl_context.vbo);
                    gl::DeleteVertexArrays(1, &drawing.gl_context.vao);
                }
                *control_flow = ControlFlow::Exit
            }
            // Mouse pressed
            // deprecated is for modifiers
            #[allow(deprecated)]
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
                modifiers: _,
            } => {
                if button == MouseButton::Left {
                    input.cursor.pressed = state == ElementState::Pressed;

                    if input.cursor.pressed == false {
                        input.cursor.released_time = Some(SystemTime::now());

                        for _ in 0..drawing.line_style.smoothing_intensity {
                            apply_line_smoothing(
                                &mut drawing.vertices[drawing.smooth_index..],
                                drawing.line_style.smoothing_range,
                            );
                        }
                        drawing.smooth_index = drawing.vertices.len();

                        drawing.need_redraw = true;
                    }
                }
            }
            // Mousewheel
            // deprecated is for modifiers
            #[allow(deprecated)]
            WindowEvent::MouseWheel {
                device_id: _,
                delta,
                phase,
                modifiers: _,
            } => {
                if phase == TouchPhase::Moved {
                    match delta {
                        MouseScrollDelta::LineDelta(_x, y) => {
                            drawing.need_redraw = true;

                            drawing.line_style.width -= y;
                            if drawing.line_style.width < 1.0 {
                                drawing.line_style.width = 1.0;
                            }
                        }
                        _ => (),
                    }
                }
            }
            // Mouse moved
            // deprecated is for modifiers
            #[allow(deprecated)]
            WindowEvent::CursorMoved {
                device_id: _,
                position,
                modifiers: _,
            } => {
                input.cursor.last_x = input.cursor.x;
                input.cursor.last_y = input.cursor.y;
                input.cursor.x = position.x as f32;
                input.cursor.y = position.y as f32;
                drawing.need_redraw = true;
            }
            _ => (),
        },
        _ => (),
    }
}

fn redraw(drawing: &mut DrawingState, input: &Input, cursor_vertices: &mut Vec<f32>) {
    let prev_cursor_gl_pos = Point {
        x: input.cursor.last_x / drawing.rect.width * 2.0 - 1.0,
        y: input.cursor.last_y / drawing.rect.height * -2.0 + 1.0,
        z: 0.0,
    };
    let cursor_gl_pos = Point {
        x: input.cursor.x / drawing.rect.width * 2.0 - 1.0,
        y: input.cursor.y / drawing.rect.height * -2.0 + 1.0,
        z: 0.0,
    };

    let cursor_gl_size = screen_size_to_gl(
        drawing.line_style.width,
        drawing.line_style.width,
        &drawing.rect,
    );
    let cursor_outline_gl_size = screen_size_to_gl(
        drawing.line_style.width + 1.0,
        drawing.line_style.width + 1.0,
        &drawing.rect,
    );

    // Cursor circle overlay
    for i in 0..N_CURSOR_RETICLE_POINTS {
        let angle = (i as f32) / (N_CURSOR_RETICLE_POINTS as f32) * (2.0 * PI);
        cursor_vertices[i * 6 + 0] = cursor_gl_pos.x + (angle.cos() * cursor_gl_size.width);
        cursor_vertices[i * 6 + 1] = cursor_gl_pos.y + (angle.sin() * cursor_gl_size.height);
        // skip z  [i * 6 + 2]
        cursor_vertices[i * 6 + 3] = drawing.line_style.color[0];
        cursor_vertices[i * 6 + 4] = drawing.line_style.color[1];
        cursor_vertices[i * 6 + 5] = drawing.line_style.color[2];
    }
    // // Cursor circle outline
    for i in N_CURSOR_RETICLE_POINTS..(N_CURSOR_RETICLE_POINTS * 2) {
        let angle = (i as f32) / (N_CURSOR_RETICLE_POINTS as f32) * (2.0 * PI);
        cursor_vertices[i * 6 + 0] = cursor_gl_pos.x + (angle.cos() * cursor_outline_gl_size.width);
        cursor_vertices[i * 6 + 1] =
            cursor_gl_pos.y + (angle.sin() * cursor_outline_gl_size.height);
        // skip z  [i * 6 + 2]
        cursor_vertices[i * 6 + 3] = 0.0;
        cursor_vertices[i * 6 + 4] = 0.0;
        cursor_vertices[i * 6 + 5] = 0.0;
    }

    if !input.cursor.pressed || drawing.is_window_hidden {
        drawing.n_points_current_line = 0;
    } else {
        /*
        Each line segment is formed of 2 triangles that form a quad

        p3 __ p4    - old cursor position
          |\ |
          | \|
        p1 ¯¯ p2    - new cursor position

        p1: current cursor position - line width
        p2: current cursor position + line width
        p3: previous cursor position - line width
        p4: previous cursor position + line width

        The cursor position is always between the two points
        p3 ____ old cursor ____ p4
          |                    |
          |                    |
          |                    |
          |                    |
        p1¯¯¯¯¯ new cursor ¯¯¯¯ p2
        */

        // Angle in radians of the line to draw
        // Will be wrong if it's the first vertex since prev_positions isn't defined
        // but we recalculate it before drawing when we get the second vertex
        let angle =
            (cursor_gl_pos.y - prev_cursor_gl_pos.y).atan2(cursor_gl_pos.x - prev_cursor_gl_pos.x);

        // New line segment, add an undo point
        if drawing.n_points_current_line == 0
            && (input.cursor.released_time.is_none()
                || input
                    .cursor
                    .released_time
                    .unwrap()
                    .elapsed()
                    .unwrap()
                    .as_millis()
                    > 200)
        {
            drawing.undo_steps.push(drawing.vertices.len());
        }

        // update line width in gl scale
        let line_gl_size = screen_size_to_gl(
            drawing.line_style.width * drawing.line_style.pressure,
            drawing.line_style.width * drawing.line_style.pressure,
            &drawing.rect,
        );

        // Previous triangles ending points (old p1.x, p1.y, p2.x, p2.y)
        let mut prev_p1 = Point {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut prev_p2 = Point {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };

        // If it's the second vertex of the line segment,
        // we need to recalculate the width of the first vertex since
        // we didnt know the angle yet
        if drawing.n_points_current_line == 1 {
            prev_p1.x = prev_cursor_gl_pos.x + (angle - FRAC_PI_2).cos() * line_gl_size.width;
            prev_p1.y = prev_cursor_gl_pos.y + (angle - FRAC_PI_2).sin() * line_gl_size.height;
            prev_p2.x = prev_cursor_gl_pos.x + (angle + FRAC_PI_2).cos() * line_gl_size.width;
            prev_p2.y = prev_cursor_gl_pos.y + (angle + FRAC_PI_2).sin() * line_gl_size.height;
        // Get the previous positions from the list of vertices
        } else if drawing.n_points_current_line > 1 {
            prev_p1.x = drawing.vertices[drawing.vertices.len() - 24];
            prev_p1.y = drawing.vertices[drawing.vertices.len() - 23];
            prev_p2.x = drawing.vertices[drawing.vertices.len() - 12];
            prev_p2.y = drawing.vertices[drawing.vertices.len() - 11];
        }

        // point to the left of the cursor
        let p1 = Point {
            x: cursor_gl_pos.x + (angle - FRAC_PI_2).cos() * line_gl_size.width,
            y: cursor_gl_pos.y + (angle - FRAC_PI_2).sin() * line_gl_size.height,
            z: 0.0,
        };

        // point to the right of the cursor
        let p2 = Point {
            x: cursor_gl_pos.x + (angle + FRAC_PI_2).cos() * line_gl_size.width,
            y: cursor_gl_pos.y + (angle + FRAC_PI_2).sin() * line_gl_size.height,
            z: 0.0,
        };

        // same position as previous p1
        let p3 = if drawing.n_points_current_line > 0 {
            prev_p1.clone()
        } else {
            // create 0 height rect for first line segment
            p1.clone()
        };

        // same position as previous p2
        let p4 = if drawing.n_points_current_line > 0 {
            prev_p2.clone()
        } else {
            // create 0 height rect for first line segment
            p2.clone()
        };

        // Triangle 3-2-1
        // 3
        drawing.vertices.extend(&(&p3).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        // 2
        drawing.vertices.extend(&(&p2).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        // 1
        drawing.vertices.extend(&(&p1).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        // Triangle 3-2-4
        // 3
        drawing.vertices.extend(&(&p3).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        // 2
        drawing.vertices.extend(&(&p2).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        // 4
        drawing.vertices.extend(&(&p4).into_array());
        drawing.vertices.extend(&drawing.line_style.color);

        drawing.n_points_current_line += 1;
    }

    if drawing.is_window_hidden {
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    } else {
        // GL Draw Phase
        unsafe {
            // Start by clearing everything from last frame
            // ClearColor has to come BEFORE Clear
            if drawing.is_background_visible {
                let bg_color_gl = color_to_gl(drawing.config.background_color);
                gl::ClearColor(
                    bg_color_gl[0],
                    bg_color_gl[1],
                    bg_color_gl[2],
                    drawing.config.background_color_opacity,
                );
            } else {
                gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            }
            gl::Clear(gl::COLOR_BUFFER_BIT);

            // Draw cursor reticle
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (cursor_vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                mem::transmute(&cursor_vertices[0]),
                gl::STATIC_DRAW,
            );

            gl::LineWidth(3.0);
            gl::DrawArrays(gl::LINE_LOOP, 0, N_CURSOR_RETICLE_POINTS as i32);
            gl::LineWidth(1.0);
            gl::DrawArrays(
                gl::LINE_LOOP,
                N_CURSOR_RETICLE_POINTS as i32,
                N_CURSOR_RETICLE_POINTS as i32,
            );

            if drawing.vertices.len() > 0 {
                // copy the vertices to the vertex buffer
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (drawing.vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                    mem::transmute(&drawing.vertices[0]),
                    gl::STATIC_DRAW,
                );

                // Draw lines using triangles to draw quads
                // Divide by 6 since each vertex has 3 floats for pos + 3 for color
                let n_line_vertices = drawing.vertices.len() / 6;
                if n_line_vertices > 0 {
                    gl::DrawArrays(gl::TRIANGLES, 0, n_line_vertices as i32);
                }
            }
        }
    }

    drawing.gl_context.window_context.swap_buffers().unwrap();
}

fn create_default_config_file() -> std::io::Result<String> {
    let mut f = std::fs::File::create("config.json").expect("Failed to create default config file");

    let default_config_json =
        serde_json::to_string_pretty(&Config::default()).expect("Failed to encode config");

    f.write_all(default_config_json.as_bytes())
        .expect("Failed to write config to file");

    Ok(default_config_json)
}

fn load_config() -> Config {
    let config_file_contents = match fs::read_to_string("config.json") {
        Err(_) => create_default_config_file(),
        Ok(r) => Ok(r),
    }
    .expect("Failed to read from config file");

    serde_json::from_str(&config_file_contents).unwrap()
}

fn color_to_gl(color: [u32; 3]) -> [f32; 3] {
    return [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
    ];
}

fn main() {
    let config = load_config();
    let event_loop = glutin::event_loop::EventLoop::new();
    let overlay_rect = get_overlay_rect(event_loop.available_monitors());
    let mut cursor_vertices = Vec::new(); // List of vertices sent to the vba. Each vertices is x, y, z, r, g, b (6 length)
    let mut drawing = DrawingState {
        need_redraw: true,            // Triggers a screen redraw when set to true
        is_window_hidden: true,       // Hide the drawing while keeping focus
        is_background_visible: false, // Toggle background color overlay
        n_points_current_line: 0,     // Number of points in the current line
        vertices: Vec::new(), // List of vertices sent to the vba. Each vertices is x, y, z, r, g, b (6 length)
        gl_context: init_gl_window(&event_loop, &overlay_rect),
        rect: overlay_rect,
        line_style: LineStyle {
            color: color_to_gl(config.brush_colors[config.default_brush_color_index as usize]), // rgb of the line to draw. Also used by the cursor reticle
            width: config.default_brush_size, // Line width to draw *in pixels*
            pressure: 1.0,                    // Used by pen pressure to change the width
            smoothing_range: config.smoothing_range,
            smoothing_intensity: config.smoothing_intensity,
        },
        undo_steps: Vec::new(), // List of indexes in vertex_data representing each possible undo steps
        smooth_index: 0,
        config: config,
    };

    // Initialize cursor reticle vertices
    // Position will be updated during event loop
    for _i in 0..N_CURSOR_RETICLE_POINTS * 2 {
        // line vertex
        cursor_vertices.push(0.0);
        cursor_vertices.push(0.0);
        cursor_vertices.push(0.0);
        // color
        cursor_vertices.extend(&drawing.line_style.color);
    }
    let mut input: Input = Default::default();

    event_loop.run(move |event, _, control_flow| {
        handle_event(event, control_flow, &mut drawing, &mut input);

        if drawing.need_redraw {
            drawing.need_redraw = false;
            redraw(&mut drawing, &input, &mut cursor_vertices);
        }
    });
}
