use gl::types::*;
use std::fs;

#[derive(Clone)]
pub struct Shader {
    id: u32,
}

impl Shader {
    pub fn new(vertex_path: &str, fragment_path: &str) -> Self {
        let shader_program: u32 =
            unsafe { Self::create_shader_program(vertex_path, fragment_path) };
        Shader { id: shader_program }
    }

    pub fn get_id(&self) -> &u32 {
        &self.id
    }

    pub fn use_shader(&self) {
        unsafe {
            gl::UseProgram(self.id);
        };
    }

    unsafe fn create_shader_program(vertex_shader_path: &str, fragment_shader_path: &str) -> u32 {
        let shader_program: u32;

        let vertex_shader_source =
            fs::read_to_string(vertex_shader_path).expect("Failed to read vertex shader source");
        let fragment_shader_source = fs::read_to_string(fragment_shader_path)
            .expect("Failed to read fragment shader source");

        let vertex_shader_cstr = std::ffi::CString::new(vertex_shader_source)
            .expect("Failed to create vertex shader CString");
        let fragment_shader_cstr = std::ffi::CString::new(fragment_shader_source)
            .expect("Failed to create fragment shader CString");

        // Compile vertex shader
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        gl::ShaderSource(
            vertex_shader,
            1,
            &vertex_shader_cstr.as_ptr(),
            std::ptr::null(),
        );
        gl::CompileShader(vertex_shader);
        Self::check_shader_compile_status(vertex_shader);

        // Compile fragment shader
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        gl::ShaderSource(
            fragment_shader,
            1,
            &fragment_shader_cstr.as_ptr(),
            std::ptr::null(),
        );
        gl::CompileShader(fragment_shader);
        Self::check_shader_compile_status(fragment_shader);

        // Link shaders and create shader program
        shader_program = gl::CreateProgram();
        gl::AttachShader(shader_program, vertex_shader);
        gl::AttachShader(shader_program, fragment_shader);
        gl::LinkProgram(shader_program);
        Self::check_shader_link_status(shader_program);

        // Cleanup
        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);

        shader_program
    }

    fn check_shader_link_status(shader: u32) {
        let mut success = gl::FALSE as GLint;
        let mut info_log = vec![0u8; 512];
        unsafe {
            gl::GetProgramiv(shader, gl::LINK_STATUS, &mut success);
            if success == gl::FALSE as GLint {
                gl::GetProgramInfoLog(
                    shader,
                    info_log.len() as GLsizei,
                    std::ptr::null_mut(),
                    info_log.as_mut_ptr() as *mut GLchar,
                );
                let error_message = std::ffi::CStr::from_ptr(info_log.as_ptr() as *const _)
                    .to_string_lossy()
                    .into_owned();
                eprintln!("ERROR::PROGRAM::LINKING_FAILED\n{}", error_message);
            }
        }
    }

    fn check_shader_compile_status(shader: u32) {
        let mut success = gl::FALSE as GLint;
        let mut info_log = vec![0u8; 512];

        unsafe {
            // Check compile status
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
            if success == gl::FALSE as GLint {
                // Retrieve error log
                gl::GetShaderInfoLog(
                    shader,
                    info_log.len() as GLsizei,
                    std::ptr::null_mut(),
                    info_log.as_mut_ptr() as *mut GLchar,
                );

                // Convert error log to a Rust string
                let error_message =
                    std::ffi::CStr::from_ptr(info_log.as_ptr() as *const gl::types::GLchar)
                        .to_string_lossy()
                        .into_owned();

                // Print error message
                eprintln!("ERROR::SHADER::COMPILATION_FAILED\n{}", error_message);
            }
        }
    }
}
