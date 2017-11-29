extern crate gleam;
extern crate glutin;
extern crate webrender;
extern crate env_logger;
extern crate euclid;

use gleam::gl;
use glutin::GlContext;
use webrender::api::*;

struct Notifier {
    window_proxy: glutin::EventsLoopProxy,
}

impl Notifier {
    fn new(window_proxy: glutin::EventsLoopProxy) -> Notifier {
        Notifier { window_proxy }
    }
}

impl RenderNotifier for Notifier {
    fn clone(&self) -> Box<RenderNotifier> {
        Box::new(Notifier {
            window_proxy: self.window_proxy.clone(),
        })
    }
    fn wake_up(&self) {
        #[cfg(not(target_os = "android"))]
        self.window_proxy.wakeup().unwrap();
    }
    fn new_document_ready(&self, _: DocumentId, _scrolled: bool, _composite_needed: bool) {
        self.wake_up();
    }
}

pub fn main() {
    env_logger::init().unwrap();

    let mut events_loop = glutin::EventsLoop::new();
    let window_builder = glutin::WindowBuilder::new()
        .with_title("WebRender test")
        .with_multitouch();
    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_gl(glutin::GlRequest::GlThenGles {
            opengl_version: (3, 2),
            opengles_version: (3, 0)
        });

    let window = glutin::GlWindow::new(window_builder, context, &events_loop).unwrap();

    unsafe {
        window.make_current().ok();
    }

    let gl = match gl::GlType::default() {
        gl::GlType::Gl => unsafe {
            gl::GlFns::load_with(|symbol| window.get_proc_address(symbol) as *const _)
        },
        gl::GlType::Gles => unsafe {
            gl::GlesFns::load_with(|symbol| window.get_proc_address(symbol) as *const _)
        },
    };

    let device_pixel_ratio = window.hidpi_factor();
    let opts = webrender::RendererOptions {
        debug: true,
        precache_shaders: false,
        device_pixel_ratio,
        clear_color: Some(ColorF::new(0.3, 0.0, 0.0, 1.0)),
        ..webrender::RendererOptions::default()
    };

    let mut framebuffer_size = {
        let (width, height) = window.get_inner_size().unwrap();
        DeviceUintSize::new(width, height)
    };
    let notifier = Box::new(Notifier::new(events_loop.create_proxy()));
    let (mut renderer, sender) = webrender::Renderer::new(gl.clone(), notifier, opts).unwrap();
    let api = sender.create_api();
    let document_id = api.add_document(framebuffer_size, 0);

    let epoch = Epoch(0);
    let pipeline_id = PipelineId(0, 0);
    let layout_size = framebuffer_size.to_f32() / euclid::ScaleFactor::new(device_pixel_ratio);
    let mut builder = DisplayListBuilder::new(pipeline_id, layout_size);
    let mut resources = ResourceUpdates::new();

    render(&api, &mut builder, &mut resources, framebuffer_size, pipeline_id, document_id);
    api.set_display_list(document_id, epoch, None, layout_size, builder.finalize(), true, resources);
    api.set_root_pipeline(document_id, pipeline_id);
    api.generate_frame(document_id, None);

    loop {
        let mut events = Vec::new();
        events_loop.run_forever(|event| {
            events.push(event);
            glutin::ControlFlow::Break
        });
        events_loop.poll_events(|event| {
            events.push(event);
        });

        for event in events {
            let mut redraw = false;
            if let glutin::Event::WindowEvent { event, .. } = event {
                match event {
                    glutin::WindowEvent::Closed |
                    glutin::WindowEvent::KeyboardInput { input: glutin::KeyboardInput {virtual_keycode: Some(glutin::VirtualKeyCode::Escape), .. }, .. } => {
                        renderer.deinit();
                        return;
                    },
                    glutin::WindowEvent::KeyboardInput { input: glutin::KeyboardInput {virtual_keycode: Some(glutin::VirtualKeyCode::P), state: glutin::ElementState::Pressed, ..}, .. } => {
                        renderer.toggle_debug_flags(webrender::DebugFlags::PROFILER_DBG);
                    },
                    glutin::WindowEvent::Resized(w, h) => {
                        window.resize(w, h);
                        framebuffer_size = DeviceUintSize::new(w, h);
                        let rect = DeviceUintRect::new(DeviceUintPoint::zero(), framebuffer_size);
                        api.set_window_parameters(document_id, framebuffer_size, rect, device_pixel_ratio);
                        redraw = true;
                    }, _ => ()
                }
                if redraw {
                    let mut builder = DisplayListBuilder::new(pipeline_id, layout_size);
                    let mut resources = ResourceUpdates::new();

                    render(
                        &api,
                        &mut builder,
                        &mut resources,
                        framebuffer_size,
                        pipeline_id,
                        document_id,
                    );
                    api.set_display_list(
                        document_id,
                        epoch,
                        None,
                        layout_size,
                        builder.finalize(),
                        true,
                        resources,
                    );
                    api.generate_frame(document_id, None);
                }
            }
        }

        renderer.update();
        renderer.render(framebuffer_size).unwrap();
        window.swap_buffers().ok();
    }
}

fn render(_api: &RenderApi,
          builder: &mut DisplayListBuilder,
          _resources: &mut ResourceUpdates,
          framebuffer_size: DeviceUintSize,
          _pipeline_id: PipelineId,
          _document_id: DocumentId) {
    let layout_size = LayoutSize::new(framebuffer_size.width as f32, framebuffer_size.height as f32);
    let bounds = LayoutRect::new(LayoutPoint::zero(), layout_size);
    builder.push_stacking_context(
        &PrimitiveInfo::new(bounds),
        ScrollPolicy::Scrollable,
        None,
        TransformStyle::Flat,
        None,
        MixBlendMode::Normal,
        Vec::new());

    push_ellipse(builder, bounds, ColorF::new(1.0, 1.0, 1.0, 1.0));

    builder.pop_stacking_context();
}

fn push_ellipse(builder: &mut DisplayListBuilder, rect: LayoutRect, color: ColorF) {
    let clip_region = ComplexClipRegion::new(rect, BorderRadius::uniform_size(rect.size / 2.0), ClipMode::Clip);
    let clip = LocalClip::RoundedRect(rect, clip_region);
    let info = PrimitiveInfo::with_clip(rect, clip);
    builder.push_rect(&info, color.into());
}
