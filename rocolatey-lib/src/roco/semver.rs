fn my_semver_get_v_part(v: &str) -> i32 {
    let v_parts: Vec<&str> = v.split("-").collect();
    match v_parts.get(0) {
        Some(v) => v.parse::<i32>().unwrap_or(0),
        None => 0,
    }
}

fn my_semver_is_newer(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split(".").collect();
    let b_parts: Vec<&str> = b.split(".").collect();

    for (i, v_a) in a_parts.iter().enumerate() {
        if b_parts.len() <= i {
            return true;
        }
        let v_b = b_parts.get(i).unwrap();
        let n_a = v_a.parse::<i32>();
        let n_b = v_b.parse::<i32>();
        if n_a.is_ok() && n_b.is_err() {
            // a is digit, b is something else
            return n_a.unwrap() >= my_semver_get_v_part(v_b);
        }
        if n_a.is_err() && n_b.is_ok() {
            // a is not a digit, but b is
            return my_semver_get_v_part(v_a) > n_b.unwrap();
        }
        if n_a.is_ok() && n_b.is_ok() {
            let n_a = n_a.unwrap();
            let n_b = n_b.unwrap();
            if n_a > n_b {
                return true;
            }
            if n_b > n_a {
                return false;
            }
            continue;
        }
        if n_a.is_err() && n_b.is_err() {
            // string compare
            return v_a > b_parts.get(i).unwrap();
        }
    }
    return false;
}

pub fn is_newer(a: &str, b: &str) -> bool {
    my_semver_is_newer(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_semver_get_v_part_test() {
        assert_eq!(my_semver_get_v_part("1"), 1);
        assert_eq!(my_semver_get_v_part("2-something"), 2);
    }

    #[test]
    fn semver_is_newer_test() {
        assert_eq!(2 + 2, 4);
        assert!(is_newer("2.1.0", "1.0.0"));
        assert!(is_newer("2.0.2", "1.12.0"));
        assert!(is_newer("1.2.3.4", "1.2.3"));
        assert!(is_newer("1.2.3.1-beta1", "1.2.3"));
        assert!(!is_newer("1.2.3.1-beta1", "1.2.3.2"));
        assert!(is_newer("1.2.3.2", "1.2.3.1-beta1"));
        assert!(is_newer("1.1.0-alpha", "1.0.0"));
        assert!(is_newer("1.11-alpha", "1.07"));
        assert!(is_newer("1.11", "1.07-alpha"));
        assert!(is_newer("1.1.0-beta2", "1.1.0-beta1"));
        assert!(is_newer("1.1.0-beta2", "1.1.0-beta14")); // this is weird, but matches choco behavior
        assert!(!is_newer("1.12.0", "2.0.2"));
        assert!(!is_newer("1.3.0", "2.1.0"));
        assert!(!is_newer("1.2.3", "1.2.3.1"));
        assert!(!is_newer("1.1.0-alpha", "1.1.0"));
        assert!(is_newer("1.1.0", "1.1.0-alpha"));
    }
}
