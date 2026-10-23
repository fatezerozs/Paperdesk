use crate::{
    db::{self, Store},
    model::FontItem,
};
pub const DEFAULT_FONT: &str = "builtin-sans";
pub static DEFAULT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansSC.ttf");
pub fn list(store: &Store) -> Result<Vec<FontItem>, String> {
    let conn = store.db.lock().map_err(|_| "数据库暂不可用")?;
    let mut items = vec![FontItem {
        id: DEFAULT_FONT.into(),
        name: "拾题黑体（内置）".into(),
    }];
    items.extend(db::all::<FontItem>(&conn, "fonts")?);
    Ok(items)
}
pub fn load(store: &Store, id: &str) -> Result<Vec<u8>, String> {
    if id == DEFAULT_FONT {
        return Ok(DEFAULT_BYTES.to_vec());
    }
    let uuid = uuid::Uuid::parse_str(id).map_err(|_| "字体 ID 无效")?;
    let conn = store.db.lock().map_err(|_| "数据库暂不可用")?;
    let _: FontItem = db::get(&conn, "fonts", &uuid.to_string())?;
    std::fs::read(store.dir.join("fonts").join(format!("{uuid}.ttf")))
        .map_err(|_| "字体文件不存在".into())
}
pub fn validate(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 24 * 1024 * 1024 {
        return Err("字体不能超过 24 MB".into());
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| "无法读取字体，请上传静态 TrueType TTF 字体")?;
    if face
        .glyph_index('_')
        .and_then(|g| face.glyph_hor_advance(g))
        .is_none_or(|width| width == 0)
    {
        return Err("字体缺少可排版的下划线字符".into());
    }
    if face.is_variable() || face.tables().cff.is_some() {
        return Err(
            "当前原生 PDF 引擎支持静态 TrueType TTF；请先转换可变字体或 OTF/CFF 字体".into(),
        );
    }
    if "拾题参考答案与解析姓名日期共难度："
        .chars()
        .any(|c| face.glyph_index(c).is_none())
    {
        return Err("请使用包含中文字符的字体，以保证题号和中文页眉可以正常显示".into());
    }
    Ok(())
}
