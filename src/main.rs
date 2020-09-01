#![windows_subsystem = "windows"]

extern crate gl;
extern crate glutin;

use gl::types::*;
use std::f64::consts::{FRAC_PI_2, PI};
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::str;

use glutin::dpi::{PhysicalPosition, PhysicalSize};
use glutin::event::{ElementState, Event, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use glutin::event_loop::ControlFlow;

// Shader sources
static VS_SRC: &'static str = include_str!("shader.vert");
static FS_SRC: &'static str = include_str!("shader.frag");

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
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
            panic!("{}", str::from_utf8(&buf).ok().expect("ShaderInfoLog not valid utf8"));
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
            gl::GetProgramInfoLog(program, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
            panic!("{}", str::from_utf8(&buf).ok().expect("ProgramInfoLog not valid utf8"));
        }
        program
    }
}

fn get_gl_size(size: f64, overlay_size: PhysicalSize<u32>) -> PhysicalSize<f64> {
    let window_height_ratio = (overlay_size.width as f64) / (overlay_size.height as f64);
    let w = size / (overlay_size.width as f64) * 2.0;
    PhysicalSize::new(w, w * window_height_ratio)
}

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();

    let mut min_x = 0;
    let mut min_y = 0;
    let mut total_width = 0;
    let mut total_height = 0;

    let monitors = event_loop.available_monitors();

    for monitor in monitors {
        if monitor.position().x < min_x {
            min_x = monitor.position().x;
        }
        if monitor.position().y < min_y {
            min_y = monitor.position().y;
        }
        if monitor.size().height > total_height {
            total_height = monitor.size().height;
        }
        total_width += monitor.size().width;
    }

    total_height += min_y.abs() as u32;

    let overlay_size: PhysicalSize<u32> = PhysicalSize::new(total_width, total_height);
    let overlay_position = PhysicalPosition::new(min_x, min_y);

    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Inke")
        .with_inner_size(overlay_size)
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_visible(false);

    let gl_window = glutin::ContextBuilder::new()
        .with_multisampling(8)
        .build_windowed(window_builder, &event_loop)
        .unwrap();

    let gl_window = unsafe { gl_window.make_current() }.unwrap();

    gl_window.window().set_outer_position(overlay_position);
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
        gl::BindFragDataLocation(program, 0, CString::new("out_color").unwrap().as_ptr());

        // position attrib
        let pos_attr = gl::GetAttribLocation(program, CString::new("position").unwrap().as_ptr());
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
        let color_attr = gl::GetAttribLocation(program, CString::new("vColor").unwrap().as_ptr());
        gl::EnableVertexAttribArray(color_attr as GLuint);
        gl::VertexAttribPointer(
            color_attr as GLuint,                                         // index of attribute
            3,                                                            // the number of components
            gl::FLOAT,                                                    // data type
            gl::FALSE as GLboolean,                                       // normalized
            (6 * std::mem::size_of::<f32>()) as gl::types::GLint,         // stride (byte offset)
            (3 * std::mem::size_of::<f32>()) as *const gl::types::GLvoid, // offset of the first component
        );
    }

    let n_cursor_reticle_points = 32;
    let mut vertex_data = Vec::new(); // List of vertices sent to the vba. Each vertices is x, y, z, r, g, b (6 length)
    let mut current_color = [1.0_f32, 1.0_f32, 1.0_f32]; // rgb of the line to draw. Also used by the cursor reticle
    let mut n_current_line_vertex = 0; // Number of vertices in the current line
    let mut pen_is_down = false; // Draw lines when true
    let mut line_width = 5.0; // Line width to draw *in pixels*
    let mut line_width_modifier = 1.0; // Used by pen pressure to change the line_width
    let mut prev_positions = [0.0_f64; 6]; // Previous triangles ending points and previous cursor (old p1.x, p1.y, p2.x, p2.y, cursor.x, cursor.y)
    let mut cursor_position = PhysicalPosition::new(0.0, 0.0); // Will hold mouse or tablet position
    let mut need_redraw = false; // Triggers a screen redraw when set to true
    let mut undo_steps: Vec<usize> = Vec::new(); // List of indexes in vertex_data representing each possible undo steps
    let mut ctrl_is_down = false; // Is the Ctrl key currently pressed
    let mut is_window_hidden = true; // Hide the drawing while keeping focus
    let mut is_background_visible = false; // Toggle background color overlay

    // Initialize cursor reticle vertices
    for _i in 0..n_cursor_reticle_points {
        // line vertex
        vertex_data.push(0.0);
        vertex_data.push(0.0);
        vertex_data.push(0.0);

        // color
        vertex_data.extend(&current_color);
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                // Alt-tab in and out
                WindowEvent::Focused(has_focus) => {
                    if has_focus {
                        // unhide
                        is_window_hidden = false;
                    } else {
                        // force window to minimize
                        gl_window.window().set_minimized(true);
                    }
                }
                WindowEvent::ModifiersChanged(modifier) => {
                    ctrl_is_down = modifier.ctrl();
                }
                WindowEvent::KeyboardInput {
                    device_id: _,
                    input,
                    is_synthetic: _,
                } => {
                    if input.state == glutin::event::ElementState::Released {
                        // println!("{}", input.scancode);

                        match input.scancode {
                            // escape
                            1 => {
                                // Todo: Request close event
                                unsafe {
                                    gl::DeleteProgram(program);
                                    gl::DeleteShader(fs);
                                    gl::DeleteShader(vs);
                                    gl::DeleteBuffers(1, &vbo);
                                    gl::DeleteVertexArrays(1, &vao);
                                }
                                *control_flow = ControlFlow::Exit
                            }
                            // h
                            35 => {
                                need_redraw = true;
                                is_window_hidden = !is_window_hidden;
                            }
                            // b
                            48 => {
                                // Toggle background
                                need_redraw = true;
                                is_background_visible = !is_background_visible;
                            }
                            // spacebar
                            57 => {
                                // Clear drawings
                                need_redraw = true;
                                vertex_data.resize(n_cursor_reticle_points * 6, 0.0);
                                n_current_line_vertex = 0;
                            }
                            // z
                            44 => {
                                // ctrl-z
                                if ctrl_is_down {
                                    // Undo (if any undo steps are available)
                                    match undo_steps.pop() {
                                        Some(n) => {
                                            vertex_data.resize(n, 0.0);
                                            need_redraw = true;
                                            n_current_line_vertex = 0;
                                        }
                                        None => (),
                                    }
                                }
                            }

                            // q,w,e,r,... for line colors

                            // q (white)
                            16 => {
                                current_color[0] = 1.0;
                                current_color[1] = 1.0;
                                current_color[2] = 1.0;
                                need_redraw = true;
                            }
                            // w (black)
                            17 => {
                                current_color[0] = 0.05;
                                current_color[1] = 0.05;
                                current_color[2] = 0.05;
                                need_redraw = true;
                            }
                            // e (orange)
                            18 => {
                                current_color[0] = 1.0;
                                current_color[1] = 0.58;
                                current_color[2] = 0.0;
                                need_redraw = true;
                            }
                            // e (pink)
                            19 => {
                                current_color[0] = 1.0;
                                current_color[1] = 0.0;
                                current_color[2] = 0.86;
                                need_redraw = true;
                            }
                            // r (red)
                            20 => {
                                current_color[0] = 1.0;
                                current_color[1] = 0.2;
                                current_color[2] = 0.2;
                                need_redraw = true;
                            }
                            // t (green)
                            21 => {
                                current_color[0] = 0.1;
                                current_color[1] = 1.0;
                                current_color[2] = 0.3;
                                need_redraw = true;
                            }
                            // y (blue)
                            22 => {
                                current_color[0] = 0.1;
                                current_color[1] = 0.3;
                                current_color[2] = 1.0;
                                need_redraw = true;
                            }
                            // u (yellow)
                            23 => {
                                current_color[0] = 1.0;
                                current_color[1] = 1.0;
                                current_color[2] = 0.0;
                                need_redraw = true;
                            }

                            // 1,2,3,... for size
                            2 => {
                                line_width = 1.0;
                                need_redraw = true;
                            }
                            3 => {
                                line_width = 3.0;
                                need_redraw = true;
                            }
                            4 => {
                                line_width = 5.0;
                                need_redraw = true;
                            }
                            5 => {
                                line_width = 10.0;
                                need_redraw = true;
                            }
                            6 => {
                                line_width = 30.0;
                                need_redraw = true;
                            }

                            _ => (),
                        }
                    }
                }
                WindowEvent::Touch(touch_event) => {
                    need_redraw = true;

                    if touch_event.phase == TouchPhase::Started {
                        pen_is_down = true;
                        n_current_line_vertex = 0;
                    }
                    if touch_event.phase == TouchPhase::Ended || touch_event.phase == TouchPhase::Cancelled {
                        pen_is_down = false;
                    }

                    cursor_position = touch_event.location;

                    match touch_event.force {
                        Some(force_type) => match force_type {
                            glutin::event::Force::Calibrated {
                                force,
                                max_possible_force,
                                altitude_angle: _,
                            } => {
                                line_width_modifier = force / max_possible_force;
                            }
                            glutin::event::Force::Normalized(force) => {
                                line_width_modifier = force;
                            }
                        },
                        None => (),
                    }
                }
                WindowEvent::CloseRequested => {
                    unsafe {
                        gl::DeleteProgram(program);
                        gl::DeleteShader(fs);
                        gl::DeleteShader(vs);
                        gl::DeleteBuffers(1, &vbo);
                        gl::DeleteVertexArrays(1, &vao);
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
                        pen_is_down = state == ElementState::Pressed
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
                                need_redraw = true;

                                line_width -= y as f64;
                                if line_width < 1.0 {
                                    line_width = 1.0;
                                }
                                if line_width > 15.0 {
                                    line_width = 30.0;
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
                    cursor_position = position;
                    need_redraw = true;
                }
                _ => (),
            },
            _ => (),
        }

        if need_redraw {
            need_redraw = false;

            let cursor = PhysicalPosition::new(
                cursor_position.x / (overlay_size.width as f64) * 2.0 - 1.0,
                cursor_position.y / (overlay_size.height as f64) * -2.0 + 1.0,
            );

            // update line width in gl scale
            let line_gl_size = get_gl_size(line_width * line_width_modifier, overlay_size);
            let cursor_gl_size = get_gl_size(line_width, overlay_size);

            // Cursor circle overlay
            for i in 0..n_cursor_reticle_points {
                let angle = (i as f64) / 32.0 * (2.0 * PI);
                vertex_data[i * 6 + 0] = (cursor.x + (angle.cos() * cursor_gl_size.width)) as f32;
                vertex_data[i * 6 + 1] = (cursor.y + (angle.sin() * cursor_gl_size.height)) as f32;
                // skip z  [i * 6 + 2]
                vertex_data[i * 6 + 3] = current_color[0];
                vertex_data[i * 6 + 4] = current_color[1];
                vertex_data[i * 6 + 5] = current_color[2];
            }

            if pen_is_down && !is_window_hidden {
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
                let angle = (cursor.y - prev_positions[5]).atan2(cursor.x - prev_positions[4]);

                // If it's the second vertex of the line segment,
                // we need to recalculate the width of the first vertex since
                // we didnt know the angle yet
                if n_current_line_vertex == 1 {
                    prev_positions[0] = prev_positions[4] + (angle - FRAC_PI_2).cos() * line_gl_size.width;
                    prev_positions[1] = prev_positions[5] + (angle - FRAC_PI_2).sin() * line_gl_size.height;
                    prev_positions[2] = prev_positions[4] + (angle + FRAC_PI_2).cos() * line_gl_size.width;
                    prev_positions[3] = prev_positions[5] + (angle + FRAC_PI_2).sin() * line_gl_size.height;
                }

                // point to the left of the cursor
                let p1 = [
                    (cursor.x + (angle - FRAC_PI_2).cos() * line_gl_size.width) as f32,
                    (cursor.y + (angle - FRAC_PI_2).sin() * line_gl_size.height) as f32,
                ];

                // point to the right of the cursor
                let p2 = [
                    (cursor.x + (angle + FRAC_PI_2).cos() * line_gl_size.width) as f32,
                    (cursor.y + (angle + FRAC_PI_2).sin() * line_gl_size.height) as f32,
                ];

                // previous p1
                let p3 = [prev_positions[0] as f32, prev_positions[1] as f32];

                // previous p2
                let p4 = [prev_positions[2] as f32, prev_positions[3] as f32];

                prev_positions[0] = p1[0] as f64;
                prev_positions[1] = p1[1] as f64;
                prev_positions[2] = p2[0] as f64;
                prev_positions[3] = p2[1] as f64;
                prev_positions[4] = cursor.x as f64; // for recalculating the first vertex
                prev_positions[5] = cursor.y as f64; // for recalculating the first vertex

                if n_current_line_vertex == 0 {
                    // Skip pushing the line segment since we only have the first point available yet
                    undo_steps.push(vertex_data.len());
                } else {
                    // 3
                    vertex_data.push(p3[0]);
                    vertex_data.push(p3[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);

                    // 2
                    vertex_data.push(p2[0]);
                    vertex_data.push(p2[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);

                    // 1
                    vertex_data.push(p1[0]);
                    vertex_data.push(p1[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);

                    // 3
                    vertex_data.push(p3[0]);
                    vertex_data.push(p3[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);

                    // 2
                    vertex_data.push(p2[0]);
                    vertex_data.push(p2[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);

                    // 4
                    vertex_data.push(p4[0]);
                    vertex_data.push(p4[1]);
                    vertex_data.push(0.0);

                    vertex_data.extend(&current_color);
                }

                n_current_line_vertex += 1;
            } else {
                n_current_line_vertex = 0;
            }

            if is_window_hidden {
                unsafe {
                    gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                }
            } else {
                // GL Draw Phase
                unsafe {
                    // Start by clearing everything from last frame
                    // ClearColor has to come BEFORE Clear
                    if is_background_visible {
                        gl::ClearColor(0.0, 0.0, 0.0, 0.8);
                    } else {
                        gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                    }
                    gl::Clear(gl::COLOR_BUFFER_BIT);

                    // copy the vertices to the vertex buffer
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (vertex_data.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                        mem::transmute(&vertex_data[0]),
                        gl::STATIC_DRAW,
                    );

                    // Draw cursor reticle
                    gl::LineWidth(2.0);
                    gl::DrawArrays(gl::LINE_LOOP, 0, n_cursor_reticle_points as i32);

                    // Draw lines using triangles to draw quads
                    // Skip the first n_cursor_reticle_points points used for the cursor
                    // Divide by 6 since each vertex has 3 floats for pos + 3 for color
                    let n_line_vertices = vertex_data.len() / 6 - n_cursor_reticle_points;
                    if n_line_vertices > 0 {
                        gl::DrawArrays(gl::TRIANGLES, n_cursor_reticle_points as i32, n_line_vertices as i32);
                    }
                }
            }

            gl_window.swap_buffers().unwrap();
        }
    });
}
