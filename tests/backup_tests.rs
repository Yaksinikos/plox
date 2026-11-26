#[cfg(test)]
mod backup_tests {
    use std::fs;
    use std::path::Path;

    use jiff::civil::date;
    use plox::{backup_once_per_day, backup_path};

    #[test]
    fn test_backup_name() {
        let day = date(2026, 3, 5);
        assert_eq!(
            backup_path(Path::new("cfg/openmw.cfg"), day),
            Path::new("cfg/openmw_05-03-2026.cfg")
        );
        assert_eq!(
            backup_path(Path::new("Morrowind.ini"), day),
            Path::new("Morrowind_05-03-2026.ini")
        );
        assert_eq!(
            backup_path(Path::new("modlist"), day),
            Path::new("modlist_05-03-2026")
        );
    }

    #[test]
    fn test_first_backup_of_the_day_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("openmw.cfg");
        let day = date(2026, 11, 26);

        fs::write(&file, "content=original.esp\n").unwrap();
        let backup = backup_once_per_day(&file, day).unwrap().unwrap();
        assert_eq!(backup, tmp.path().join("openmw_26-11-2026.cfg"));
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "content=original.esp\n"
        );

        // a later save on the same day keeps the first backup
        fs::write(&file, "content=sorted.esp\n").unwrap();
        assert_eq!(backup_once_per_day(&file, day).unwrap(), None);
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "content=original.esp\n"
        );

        // the next day gets its own backup
        let next = backup_once_per_day(&file, date(2026, 11, 27))
            .unwrap()
            .unwrap();
        assert_eq!(next, tmp.path().join("openmw_27-11-2026.cfg"));
        assert_eq!(fs::read_to_string(&next).unwrap(), "content=sorted.esp\n");
    }

    #[test]
    fn test_no_backup_of_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("openmw.cfg");
        assert_eq!(
            backup_once_per_day(&file, date(2026, 11, 26)).unwrap(),
            None
        );
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}
