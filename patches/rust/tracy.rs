// DEVRE DIŞI (manifest.json içinde bu dosyaya bağlanan yama yok).
//
// Neden? Bu yama seti ilk kez 2026-11-24'te gerçek bir oyun açılışında denendi ve oyun şu hatayla durdu:
//   "call of a method the reachability analysis did not keep"
// Çağrı yığınının tepesi `GlStateManager.incrementTrackedBuffers` idi ve çağrılan `Plot.setValue` metodunun
// sanal tablo girdisi (vtable slot) boştu.
//
// Sebep: Yama, `TracyClient.createPlot` gibi metotların orijinal gövdesini değiştirir. Orijinal gövde `new Plot(...)`
// ile nesne oluşturuyordu. Yama bunu kaldırınca erişilebilirlik analizi (RTA) `Plot` sınıfını hiç örneklenmemiş sayıyor
// ve `Plot.setValue` gibi metotları derlemeye almıyor. Oysa yamanın döndürdüğü `Plot.UNAVAILABLE` nesnesi çalışma
// zamanında bu metotları çağırıyor.
//
// Yeniden açmak için: yamalanan sınıfların yine de "örneklenmiş" sayılmasını sağlamak gerekir (örneğin manifestte bir
// `news` bağlaması ile) ve her sınıf için oyun açılışı test edilmelidir. Bunun dışında bu dosyadaki `init_X()?` çağrıları
// (UNAVAILABLE okumadan önce sınıfı başlatma) doğru bir düzeltmedir ve korunmalıdır.
//
// Tracy (profilci) bu derlemenin parçası değildir.
//
// jtracy yerel metotlarını yalnızca TracyClient.load() başarılı olduğunda çağırır (`loaded`); aksi hâlde verdiği her nesne
// bir sınıfın UNAVAILABLE örneğidir ve metotları hemen döner. Burada load(), Tracy kütüphanesi olmadan JVM'in yaptığı gibi
// başarısız olur (UnsatisfiedLinkError), bu yüzden `loaded` false kalır ve yalnızca UNAVAILABLE örnekleri vardır; yerel metot
// çağıracak her jtracy metodu, o durumda yaptığı şeyle (hiçbir şey, `this` ya da X.UNAVAILABLE) tam olarak değiştirilir.
// 21 TracyBindings yerel metodu ve öykünmeleri programdan çıkar. jtracy jar'ının bytecode'undan üretildi; bu metotları
// değiştiren bir jtracy güncellemesi, parmak izleri sayesinde derlemeyi durdurur.

pub fn continuous_frame_mark_0(_this: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn discontinuous_frame_start_1(_this: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn discontinuous_frame_end_2(_this: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn gpu_context_set_name_3(this: rt::Ref, _a0: rt::Ref) -> rt::Res<rt::Ref> { Ok(this) }
pub fn gpu_context_begin_zone_4(_this: rt::Ref, _a0: i32, _a1: rt::Ref, _a2: rt::Ref, _a3: rt::Ref, _a4: i32) -> rt::Res<()> { Ok(()) }
pub fn gpu_context_end_zone_5(_this: rt::Ref, _a0: i32) -> rt::Res<()> { Ok(()) }
pub fn gpu_context_submit_query_timestamp_6(_this: rt::Ref, _a0: i32, _a1: i64) -> rt::Res<()> { Ok(()) }
pub fn memory_pool_malloc_7(_this: rt::Ref, _a0: i64, _a1: i32) -> rt::Res<()> { Ok(()) }
pub fn memory_pool_free_8(_this: rt::Ref, _a0: i64) -> rt::Res<()> { Ok(()) }
pub fn plot_set_value_9(_this: rt::Ref, _a0: f64) -> rt::Res<()> { Ok(()) }
pub fn section_close_10(_this: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn section_category_enter_section_11(_this: rt::Ref, _a0: rt::Ref) -> rt::Res<rt::Ref> { init_Section()?; Ok(unsafe { *s_section_unavailable() }) }
pub fn tracy_client_load_12() -> rt::Res<()> { rt::throw("java/lang/UnsatisfiedLinkError", "Tracy is not part of this build".to_string()) }
pub fn tracy_client_mark_frame_13() -> rt::Res<()> { Ok(()) }
pub fn tracy_client_frame_image_14(_a0: rt::Ref, _a1: i32, _a2: i32, _a3: i32, _a4: i32) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_begin_zone_15(_a0: rt::Ref, _a1: i32) -> rt::Res<rt::Ref> { init_Zone()?; Ok(unsafe { *s_zone_unavailable() }) }
pub fn tracy_client_begin_zone_16(_a0: rt::Ref, _a1: rt::Ref, _a2: rt::Ref, _a3: i32) -> rt::Res<rt::Ref> { init_Zone()?; Ok(unsafe { *s_zone_unavailable() }) }
pub fn tracy_client_set_thread_name_17(_a0: rt::Ref, _a1: i32) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_create_plot_18(_a0: rt::Ref) -> rt::Res<rt::Ref> { init_Plot()?; Ok(unsafe { *s_plot_unavailable() }) }
pub fn tracy_client_create_discontinuous_frame_19(_a0: rt::Ref) -> rt::Res<rt::Ref> { init_DiscontinuousFrame()?; Ok(unsafe { *s_discontinuous_frame_unavailable() }) }
pub fn tracy_client_create_continuous_frame_20(_a0: rt::Ref) -> rt::Res<rt::Ref> { init_ContinuousFrame()?; Ok(unsafe { *s_continuous_frame_unavailable() }) }
pub fn tracy_client_create_memory_pool_21(_a0: rt::Ref) -> rt::Res<rt::Ref> { init_MemoryPool()?; Ok(unsafe { *s_memory_pool_unavailable() }) }
pub fn tracy_client_report_app_info_22(_a0: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_message_23(_a0: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_message_24(_a0: rt::Ref, _a1: i32) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_message_25(_a0: rt::Ref) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_message_26(_a0: rt::Ref, _a1: i32) -> rt::Res<()> { Ok(()) }
pub fn tracy_client_create_gpu_context_27(_a0: rt::Ref, _a1: i64, _a2: f32) -> rt::Res<rt::Ref> { init_GpuContext()?; Ok(unsafe { *s_gpu_context_unavailable() }) }
pub fn tracy_client_create_section_category_28(_a0: rt::Ref) -> rt::Res<rt::Ref> { init_SectionCategory()?; Ok(unsafe { *s_section_category_unavailable() }) }
pub fn zone_add_text_29(this: rt::Ref, _a0: rt::Ref) -> rt::Res<rt::Ref> { Ok(this) }
pub fn zone_set_color_30(this: rt::Ref, _a0: i32) -> rt::Res<rt::Ref> { Ok(this) }
pub fn zone_add_value_31(this: rt::Ref, _a0: i64) -> rt::Res<rt::Ref> { Ok(this) }
pub fn zone_close_32(_this: rt::Ref) -> rt::Res<()> { Ok(()) }
