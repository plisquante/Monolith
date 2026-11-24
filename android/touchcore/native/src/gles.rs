//! Render donanım arayüzü (RHI) için OpenGL ES 3.0 arka ucu (yalnızca Android).
//!
//! `libGLESv3` (NDK tarafından sağlanır) üzerinde `rhi::Backend`'i gerçekleştirir. Aygıt tamponları ve boru hatları
//! çağıran tarafından oluşturulur ve burada tutamaçla kaydedilir; bu arka uç yalnızca GL durumunu kaydeder ve çizimleri komut sırasıyla yapar.
//!
//! GLES 3.0'ın yapamadığı şey yüksek sesle reddedilir (eksik özelliği adlandıran bir panic), asla atlanmaz:
//! * temel tepe (base vertex) ve temel örnek (base instance) GLES 3.2 gerektirir (`glDrawElementsInstancedBaseVertexBaseInstance`);
//! * çoklu dolaylı çizim GLES 3.1 gerektirir, bu yüzden dolaylı bir aralık CPU tarafı geri düşüş listesinden komut komut çizilir.
//!
//! Durum: yazıldı ama henüz derlenmedi ya da çalıştırılmadı. `aarch64-linux-android` için derlenmeli ve kullanılmadan önce
//! bir cihazda denetlenmelidir. Örnek başına öznitelik yerleşimi (`BindInstanceBuffer`) bir varsayımdır (`texel_stride` baytta
//! dört float bileşeni); gölgelendirici tarafının bunu doğrulaması gerekir.
#![cfg(target_os = "android")]

use crate::instancing::DrawIndexedIndirect;
use crate::rhi::{Backend, BufferHandle, Command, FrameRing, PipelineHandle};
use std::collections::HashMap;
use std::ffi::c_void;

type GLenum = u32;
type GLuint = u32;
type GLint = i32;
type GLsizei = i32;
type GLsizeiptr = isize;
type GLintptr = isize;
type GLboolean = u8;

const GL_ARRAY_BUFFER: GLenum = 0x8892;
const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
const GL_COPY_WRITE_BUFFER: GLenum = 0x8F37;
const GL_DYNAMIC_DRAW: GLenum = 0x88E8;
const GL_TRIANGLES: GLenum = 0x0004;
const GL_UNSIGNED_SHORT: GLenum = 0x1403;
const GL_UNSIGNED_INT: GLenum = 0x1405;
const GL_FLOAT: GLenum = 0x1406;
const GL_FALSE: GLboolean = 0;

#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glGenVertexArrays(n: GLsizei, arrays: *mut GLuint);
    fn glBindVertexArray(array: GLuint);
    fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
    fn glBindBuffer(target: GLenum, buffer: GLuint);
    fn glBufferData(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum);
    fn glBufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const c_void);
    fn glUseProgram(program: GLuint);
    fn glEnableVertexAttribArray(index: GLuint);
    fn glVertexAttribPointer(index: GLuint, size: GLint, ty: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void);
    fn glVertexAttribDivisor(index: GLuint, divisor: GLuint);
    fn glDrawElementsInstanced(mode: GLenum, count: GLsizei, ty: GLenum, indices: *const c_void, instance_count: GLsizei);
}

/// Bir çizim iş parçacığının GL durumu. EGL bağlamına sahip iş parçacığında oluşturun ve `new`'i yalnızca o
/// bağlam geçerliyken çağırın (her öznitelik çağrısının ihtiyaç duyduğu köşe dizisi nesnesini oluşturur).
pub struct GlesBackend {
    vao: GLuint,
    buffers: HashMap<u32, GLuint>,
    programs: HashMap<u32, GLuint>,
    index_32: bool,
}

impl GlesBackend {
    pub fn new() -> Self {
        let mut vao: GLuint = 0;
        unsafe {
            glGenVertexArrays(1, &mut vao);
            glBindVertexArray(vao);
        }
        GlesBackend { vao, buffers: HashMap::new(), programs: HashMap::new(), index_32: true }
    }

    /// Bir aygıt tamponu tutamacının arkasındaki GL tamponunu oluşturur. Tutamaç başına bir kez, onu kullanan kareden önce çağırın.
    pub fn create_buffer(&mut self, handle: BufferHandle, size_bytes: usize) {
        let mut id: GLuint = 0;
        unsafe {
            glGenBuffers(1, &mut id);
            glBindBuffer(GL_COPY_WRITE_BUFFER, id);
            glBufferData(GL_COPY_WRITE_BUFFER, size_bytes as GLsizeiptr, std::ptr::null(), GL_DYNAMIC_DRAW);
            glBindBuffer(GL_COPY_WRITE_BUFFER, 0);
        }
        if self.buffers.insert(handle.0, id).is_some() {
            panic!("gles: buffer handle {} created twice", handle.0);
        }
    }

    /// Bir aygıt boru hattı tutamacını bağlanmış bir GL programına eşler. Program çağıranın sahipliğindedir.
    pub fn register_pipeline(&mut self, handle: PipelineHandle, program: GLuint) {
        self.programs.insert(handle.0, program);
    }

    /// Bu arka ucun tüm öznitelik ve eleman durumu için bağladığı VAO.
    pub fn vertex_array(&self) -> GLuint {
        self.vao
    }

    fn buffer(&self, h: BufferHandle) -> GLuint {
        match self.buffers.get(&h.0) {
            Some(&id) => id,
            None => panic!("gles: buffer handle {} was not created", h.0),
        }
    }

    fn draw_one(&self, index_count: u32, instance_count: u32, first_index: u32, vertex_offset: i32, first_instance: u32) {
        if vertex_offset != 0 {
            panic!("gles: vertex_offset {} needs GLES 3.2 (base vertex)", vertex_offset);
        }
        if first_instance != 0 {
            panic!("gles: first_instance {} needs GLES 3.2 (base instance)", first_instance);
        }
        let (ty, bytes_per_index) = if self.index_32 { (GL_UNSIGNED_INT, 4usize) } else { (GL_UNSIGNED_SHORT, 2usize) };
        unsafe {
            glDrawElementsInstanced(
                GL_TRIANGLES,
                index_count as GLsizei,
                ty,
                (first_index as usize * bytes_per_index) as *const c_void,
                instance_count as GLsizei,
            );
        }
    }
}

impl Default for GlesBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for GlesBackend {
    fn execute(&mut self, commands: &[Command], fallback: &[DrawIndexedIndirect], ring: &FrameRing) {
        for cmd in commands {
            match *cmd {
                Command::BindPipeline(p) => {
                    let prog = match self.programs.get(&p.0) {
                        Some(&g) => g,
                        None => panic!("gles: pipeline {} was not registered", p.0),
                    };
                    unsafe { glUseProgram(prog) };
                }
                Command::BindIndexBuffer { buf, index_32 } => {
                    let id = self.buffer(buf);
                    self.index_32 = index_32;
                    unsafe { glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, id) };
                }
                Command::BindInstanceBuffer { binding, buf, offset_bytes, size_bytes: _, texel_stride } => {
                    let id = self.buffer(buf);
                    unsafe {
                        glBindBuffer(GL_ARRAY_BUFFER, id);
                        glEnableVertexAttribArray(binding);
                        glVertexAttribPointer(
                            binding,
                            4,
                            GL_FLOAT,
                            GL_FALSE,
                            texel_stride as GLsizei,
                            offset_bytes as *const c_void,
                        );
                        glVertexAttribDivisor(binding, 1);
                    }
                }
                Command::DrawIndexedInstanced { index_count, instance_count, first_index, vertex_offset, first_instance } => {
                    self.draw_one(index_count, instance_count, first_index, vertex_offset, first_instance);
                }
                Command::DrawIndexedIndirectRange { fallback: range, .. } => {
                    for d in &fallback[range.range()] {
                        self.draw_one(d.index_count, d.instance_count, d.first_index, d.vertex_offset, d.first_instance);
                    }
                }
                Command::CopyStaging { src_offset, len, dst, dst_offset } => {
                    let id = self.buffer(dst);
                    let src = &ring.bytes()[src_offset..src_offset + len];
                    unsafe {
                        glBindBuffer(GL_COPY_WRITE_BUFFER, id);
                        glBufferSubData(GL_COPY_WRITE_BUFFER, dst_offset as GLintptr, len as GLsizeiptr, src.as_ptr() as *const c_void);
                        glBindBuffer(GL_COPY_WRITE_BUFFER, 0);
                    }
                }
            }
        }
    }
}
