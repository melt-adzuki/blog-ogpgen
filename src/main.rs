use http_cache_reqwest::{Cache, HttpCache, HttpCacheOptions, MokaManager};
use once_cell::sync::Lazy;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use rocket::{http::ContentType, routes};
use rust_embed::Embed;
use skia_safe::{
    font, surfaces, utils::text_utils, Color, Data, EncodedImageFormat, Font, FontMgr, Image,
    Paint, M44,
};

#[rocket::launch]
fn rocket() -> _ {
    let rocket = rocket::build().mount("/", routes![index]);
    let figment = rocket
        .figment()
        .clone()
        .merge((rocket::Config::PORT, 14400));
    rocket.configure(figment)
}

static REQWEST: Lazy<ClientWithMiddleware> = Lazy::new(|| {
    ClientBuilder::new(reqwest::Client::new())
        .with(Cache(HttpCache {
            mode: http_cache_reqwest::CacheMode::IgnoreRules,
            manager: MokaManager::default(),
            options: HttpCacheOptions::default(),
        }))
        .build()
});

#[rocket::get("/?<img>&<ttl>&<hl>")]
async fn index(img: Option<String>, ttl: String, hl: Option<String>) -> (ContentType, Vec<u8>) {
    let bg_bytes = match img {
        Some(img) => {
            let res = (&REQWEST).get(&img).send().await.unwrap();
            res.bytes().await.unwrap()
        }
        None => Asset::get("bg_1.png")
            .unwrap()
            .data
            .as_ref()
            .to_owned()
            .into(),
    };

    let res = draw(
        &ttl,
        match &hl {
            Some(text) => OverlayType::Highlight(text),
            None => OverlayType::Normal,
        },
        &bg_bytes,
    );

    (ContentType::PNG, res)
}

const WIDTH: i32 = 1440;
const HEIGHT: i32 = 1080;

#[derive(Embed)]
#[folder = "assets/"]
struct Asset;

fn draw(text: &str, overlay_type: OverlayType, bg_bytes: &[u8]) -> Vec<u8> {
    let mut surface = surfaces::raster_n32_premul((WIDTH, HEIGHT)).unwrap();
    let canvas = surface.canvas();

    // Bytes<&[u8]> to SkData
    let bg_image = Image::from_encoded(Data::new_copy(bg_bytes)).unwrap();

    // Fit image width with canvas width keeping aspect ratio
    let width = bg_image.dimensions().width;
    let height = bg_image.dimensions().height;
    let scale = WIDTH as f32 / width as f32;
    let left_top = (0, {
        let h1 = HEIGHT as f32;
        let h2 = height as f32;
        (h1 / scale - h2) / 2.0
    } as i32);

    canvas.set_matrix(&M44::scale(scale, scale, 1.0));

    canvas.draw_image(bg_image, left_top, Option::None);
    canvas.reset_matrix();

    let overlay_f = Asset::get(match overlay_type {
        OverlayType::Normal => "overlay_1.png",
        OverlayType::Highlight(_) => "overlay_2.png",
    })
    .unwrap();
    let overlay_b = overlay_f.data.as_ref();
    let overlay_img = Image::from_encoded(Data::new_copy(&overlay_b)).unwrap();
    canvas.draw_image(overlay_img, (0, 0), Option::None);

    let font_mgr = FontMgr::new();

    let mut font = {
        let file = Asset::get("font_1.otf").unwrap();
        let bytes = file.data.as_ref();
        let typeface = font_mgr.new_from_data(&bytes, Option::None).unwrap();
        Font::from_typeface(typeface, 72.0)
    };
    font.set_edging(font::Edging::AntiAlias);

    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);

    let width = font.measure_str(text, Some(&paint)).0;

    if let OverlayType::Highlight(text) = overlay_type {
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.set_anti_alias(true);

        let mut font_highlight = {
            let file = Asset::get("font_2.otf").unwrap();
            let bytes = file.data.as_ref();
            let typeface = font_mgr.new_from_data(&bytes, Option::None).unwrap();
            Font::from_typeface(typeface, 54.0)
        };
        font_highlight.set_edging(font::Edging::AntiAlias);

        text_utils::draw_str(
            canvas,
            text,
            (292, 1004),
            &font_highlight,
            &paint,
            text_utils::Align::Center,
        );
    }

    let (threshold, center) = match overlay_type {
        OverlayType::Normal => (1200.0, 807),
        OverlayType::Highlight(_) => (1000.0, 900),
    };

    if width > threshold {
        let overflow = width - threshold;
        font.set_scale_x(1.0 - overflow / width);
    }

    text_utils::draw_str(
        canvas,
        text,
        (center, 1010),
        &font,
        &paint,
        text_utils::Align::Center,
    );

    let image = surface.image_snapshot();
    let data = image
        .encode(Option::None, EncodedImageFormat::PNG, Option::None)
        .unwrap();

    return data.as_bytes().to_owned();
}

#[derive(PartialEq)]
enum OverlayType<'a> {
    Normal,
    Highlight(&'a String),
}
