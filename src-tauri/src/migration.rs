use std::fs;
use std::path::PathBuf;

/// 从旧数据目录 ~/.ai-tool-manager/ 迁移到 ~/.ai-toolkit/
/// 必须在 Database::new() 之前调用
pub fn migrate_from_old_dir() {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };

    let old_dir = home.join(".ai-tool-manager");
    let new_dir = home.join(".ai-toolkit");

    // 旧目录不存在，无需迁移
    if !old_dir.exists() {
        return;
    }

    // 新目录已存在且非空，说明用户已用过新版，跳过迁移
    if new_dir.exists() && is_dir_non_empty(&new_dir) {
        return;
    }

    eprintln!("[migration] 检测到旧数据目录 ~/.ai-tool-manager/，开始迁移到 ~/.ai-toolkit/");

    // 1. 创建新目录
    if let Err(e) = fs::create_dir_all(&new_dir) {
        eprintln!("[migration] 创建新目录失败: {}", e);
        return;
    }

    // 2. 迁移数据库文件
    let old_db = old_dir.join("ai-tool-manager.db");
    let new_db = new_dir.join("ai-toolkit.db");
    if old_db.exists() && !new_db.exists() {
        match fs::copy(&old_db, &new_db) {
            Ok(_) => {
                eprintln!("[migration] 数据库文件已复制");
                // 修正数据库内部的旧路径
                fix_db_paths(&new_db);
            }
            Err(e) => {
                eprintln!("[migration] 数据库文件复制失败: {}", e);
            }
        }
    }

    // 3. 移动 detected.json
    let old_detected = old_dir.join("detected.json");
    let new_detected = new_dir.join("detected.json");
    if old_detected.exists() && !new_detected.exists() {
        if let Err(e) = fs::rename(&old_detected, &new_detected) {
            // rename 可能跨文件系统失败，尝试复制+删除
            if let Err(e2) = fs::copy(&old_detected, &new_detected).and_then(|_| fs::remove_file(&old_detected)) {
                eprintln!("[migration] detected.json 迁移失败: rename={}, copy={}", e, e2);
            } else {
                eprintln!("[migration] detected.json 已迁移");
            }
        } else {
            eprintln!("[migration] detected.json 已迁移");
        }
    }

    // 4. 移动 skills/ 目录
    let old_skills = old_dir.join("skills");
    let new_skills = new_dir.join("skills");
    if old_skills.exists() && !new_skills.exists() {
        if let Err(e) = fs::rename(&old_skills, &new_skills) {
            // rename 可能跨文件系统失败，尝试递归复制
            match copy_dir_recursive(&old_skills, &new_skills) {
                Ok(_) => {
                    let _ = fs::remove_dir_all(&old_skills);
                    eprintln!("[migration] skills/ 目录已迁移");
                }
                Err(e2) => {
                    eprintln!("[migration] skills/ 目录迁移失败: rename={}, copy={}", e, e2);
                }
            }
        } else {
            eprintln!("[migration] skills/ 目录已迁移");
        }
    }

    // 5. 备份旧目录
    backup_old_dir(&old_dir);

    eprintln!("[migration] 迁移完成");
}

/// 检查目录是否非空
fn is_dir_non_empty(dir: &PathBuf) -> bool {
    match fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

/// 修正数据库中存储的旧路径
fn fix_db_paths(db_path: &PathBuf) {
    match rusqlite::Connection::open(db_path) {
        Ok(conn) => {
            // managed_skills.central_path 中包含旧路径
            let _ = conn.execute(
                "UPDATE managed_skills SET central_path = REPLACE(central_path, '.ai-tool-manager', '.ai-toolkit')",
                [],
            );
            // skill_sync_targets.target_path 中包含旧路径
            let _ = conn.execute(
                "UPDATE skill_sync_targets SET target_path = REPLACE(target_path, '.ai-tool-manager', '.ai-toolkit')",
                [],
            );
            eprintln!("[migration] 数据库内部路径已修正");
        }
        Err(e) => {
            eprintln!("[migration] 打开数据库修正路径失败: {}", e);
        }
    }
}

/// 将旧目录重命名为 .ai-tool-manager.bak/，如已存在则加数字后缀
fn backup_old_dir(old_dir: &PathBuf) {
    let home = match old_dir.parent() {
        Some(h) => h,
        None => return,
    };

    let mut bak_dir = home.join(".ai-tool-manager.bak");
    let mut suffix = 1;
    while bak_dir.exists() {
        bak_dir = home.join(format!(".ai-tool-manager.bak.{}", suffix));
        suffix += 1;
    }

    if let Err(e) = fs::rename(old_dir, &bak_dir) {
        eprintln!("[migration] 备份旧目录失败: {}", e);
    } else {
        eprintln!("[migration] 旧目录已备份为 {}", bak_dir.display());
    }
}

/// 递归复制目录
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<(), std::io::Error> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
