// https://github.com/Nercury/rust-and-opengl-lessons

extern crate gl;
extern crate glutin;

use gl::types::*;
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::str;

use glutin::dpi::PhysicalPosition;
use glutin::dpi::PhysicalSize;
use glutin::event::MouseButton;
use glutin::event::ElementState;
use glutin::event::TouchPhase;
use glutin::event::MouseScrollDelta;

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

fn main() {
    let screen_position: PhysicalPosition<i32> = PhysicalPosition::new(-5, 0);
    let screen_size: PhysicalSize<u32> = PhysicalSize::new(1920, 1079);

    let mut vertex_data = vec![0.0; 64];
    let event_loop = glutin::event_loop::EventLoop::new();
    let mut first_draw = true;
    let mut mouse_is_down = false;
    let mut line_width = 5.0;
    let mut prev_cursor_x = 0.0;
    let mut prev_cursor_y = 0.0;

    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Whiteboard")
        .with_inner_size(screen_size)
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_visible(false)
        .with_always_on_top(true);
        
    let gl_window = glutin::ContextBuilder::new()
        .with_multisampling(8)
        .with_vsync(true)
        .build_windowed(window_builder, &event_loop)
        .unwrap();
        
    let gl_window = unsafe { gl_window.make_current() }.unwrap();

    gl_window.window().set_outer_position(screen_position);
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

        // Create a Vertex Buffer Object
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

        // Use shader program
        gl::UseProgram(program);
        gl::BindFragDataLocation(program, 0, CString::new("out_color").unwrap().as_ptr());

        // Specify the layout of the vertex data
        let pos_attr = gl::GetAttribLocation(program, CString::new("position").unwrap().as_ptr());
        gl::EnableVertexAttribArray(pos_attr as GLuint);
        gl::VertexAttribPointer(
            pos_attr as GLuint,
            2,
            gl::FLOAT,
            gl::FALSE as GLboolean,
            0,
            ptr::null(),
        );
    }

    // unsafe {
    //     gl::ClearColor(0.0,0.0,0.0,0.0);
    //     gl::Clear(gl::COLOR_BUFFER_BIT);
    // }
    // gl_window.swap_buffers().unwrap();

    event_loop.run(move |event, _, control_flow| {
        use glutin::event::{Event, WindowEvent};
        use glutin::event_loop::ControlFlow;
        *control_flow = ControlFlow::Wait;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                // WindowEvent::Resized(physical_size) => {
                //     gl_window.resize(physical_size);
                // }
                // WindowEvent::Focused(_has_focus) => {
                //     println!("Focused");
                // }
                // WindowEvent::Moved(_position) => {
                //     println!("Moved");
                // }
                // WindowEvent::ModifiersChanged(_state) => {
                //     println!("Modifiers changed");
                // }
                WindowEvent::CloseRequested => {
                    // Cleanup
                    unsafe {
                        gl::DeleteProgram(program);
                        gl::DeleteShader(fs);
                        gl::DeleteShader(vs);
                        gl::DeleteBuffers(1, &vbo);
                        gl::DeleteVertexArrays(1, &vao);
                    }
                    *control_flow = ControlFlow::Exit
                },
                #[allow(deprecated)]
                WindowEvent::MouseInput {device_id: _, state, button, modifiers: _} => {
                    if button == MouseButton::Left {
                        mouse_is_down = state == ElementState::Pressed
                    }
                }
                #[allow(deprecated)]
                WindowEvent::MouseWheel {device_id: _, delta, phase, modifiers: _} => {
                    if phase == TouchPhase::Moved {
                        match delta {
                            MouseScrollDelta::LineDelta(_x, y) => {
                                line_width -= y as f64;
                                if line_width < 1.0 { line_width = 1.0; }
                                if line_width > 10.0 { line_width = 10.0; }
                            }
                            _ => ()
                        }
                    }
                }
                #[allow(deprecated)]
                WindowEvent::CursorMoved {device_id: _, position, modifiers: _} => {
                    let window_size = gl_window.window().inner_size();
                    let cursor_display_size = line_width / (window_size.width as f64) * 2.0;
                    let height_ratio = (window_size.width as f64) / (window_size.height as f64);
                    let cursor_x = position.x / (window_size.width as f64) * 2.0 - 1.0;
                    let cursor_y = position.y / (window_size.height as f64) * -2.0 + 1.0;

                    // Cursor circle overlay
                    for i in 0..32 {
                        let angle = (i as f64) / 32.0 * 360.0 * 3.14159 / 180.0;
                        vertex_data[i * 2 + 0] = (cursor_x + (angle.cos() * cursor_display_size)) as f32;
                        vertex_data[i * 2 + 1] = (cursor_y + (angle.sin() * (cursor_display_size * height_ratio))) as f32;
                    }

                    if mouse_is_down {
                        if first_draw {
                            first_draw = false;
                        } else {
                            vertex_data.push(prev_cursor_x as f32);
                            vertex_data.push(prev_cursor_y as f32);
                            vertex_data.push(cursor_x as f32);
                            vertex_data.push(cursor_y as f32);
                        }
                        prev_cursor_x = cursor_x;
                        prev_cursor_y = cursor_y;
                    } else {
                        first_draw = true;
                    }

                    unsafe { 
                        gl::ClearColor(0.0,0.0,0.0,0.0);
                        gl::Clear(gl::COLOR_BUFFER_BIT);

                        // copy the vertex data to the vertex buffer
                        gl::BufferData(
                            gl::ARRAY_BUFFER,
                            (vertex_data.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                            mem::transmute(&vertex_data[0]),
                            gl::STATIC_DRAW,
                        );

                        // Draw cursor circle overlay
                        gl::LineWidth(1.0);
                        gl::DrawArrays(gl::LINE_LOOP, 0, 32);

                        // Draw lines
                        let n_lines = vertex_data.len() - 64;
                        if n_lines > 0 {
                            gl::LineWidth(line_width as f32);
                            gl::DrawArrays(gl::LINES, 32, n_lines as i32);
                        }
                        
                    }

                    gl_window.swap_buffers().unwrap();
                }
                _ => (),
            },
            _ => (),
        }
    });
}
