use core::path::Path;
use std::map::HashMap;
use std::net::url;
use std::net::url::Url;

/**
Create a URL object from a string. Does various helpful browsery things like

* If there's no current url and the path looks like a file then it will
  create a file url based of the current working directory
* If there's a current url and the new path is relative then the new url
  is based off the current url

*/
#[allow(non_implicitly_copyable_typarams)]
pub fn make_url(str_url: ~str, current_url: Option<Url>) -> Url {
    let mut schm = url::get_scheme(str_url);
    let str_url = if result::is_err(&schm) {
        if current_url.is_none() {
            // If all we have is a filename, assume it's a local relative file
            // and build an absolute path with the cwd
            ~"file://" + os::getcwd().push(str_url).to_str()
        } else {
            let current_url = current_url.get();
            debug!("make_url: current_url: %?", current_url);
            if current_url.path.is_empty() || current_url.path.ends_with("/") {
                current_url.scheme + "://" + current_url.host + "/" + str_url
            } else {
                let path = str::split_char(current_url.path, '/');
                let path = path.init();
                let path = str::connect(path + ~[move str_url], "/");

                current_url.scheme + "://" + current_url.host + path
            }
        }
    } else {
        move str_url
    };

    // FIXME: Need to handle errors
    url::from_str(str_url).get()
}

mod make_url_tests {

    #[test]
    fn should_create_absolute_file_url_if_current_url_is_none_and_str_url_looks_filey() {
        let file = ~"local.html";
        let url = make_url(move file, None);
        debug!("url: %?", url);
        assert url.scheme == ~"file";
        assert url.path.contains(os::getcwd().to_str());
    }

    #[test]
    fn should_create_url_based_on_old_url_1() {
        let old_str = ~"http://example.com";
        let old_url = make_url(move old_str, None);
        let new_str = ~"index.html";
        let new_url = make_url(move new_str, Some(move old_url));
        assert new_url.scheme == ~"http";
        assert new_url.host == ~"example.com";
        assert new_url.path == ~"/index.html";
    }

    #[test]
    fn should_create_url_based_on_old_url_2() {
        let old_str = ~"http://example.com/";
        let old_url = make_url(move old_str, None);
        let new_str = ~"index.html";
        let new_url = make_url(move new_str, Some(move old_url));
        assert new_url.scheme == ~"http";
        assert new_url.host == ~"example.com";
        assert new_url.path == ~"/index.html";
    }

    #[test]
    fn should_create_url_based_on_old_url_3() {
        let old_str = ~"http://example.com/index.html";
        let old_url = make_url(move old_str, None);
        let new_str = ~"crumpet.html";
        let new_url = make_url(move new_str, Some(move old_url));
        assert new_url.scheme == ~"http";
        assert new_url.host == ~"example.com";
        assert new_url.path == ~"/crumpet.html";
    }

    #[test]
    fn should_create_url_based_on_old_url_4() {
        let old_str = ~"http://example.com/snarf/index.html";
        let old_url = make_url(move old_str, None);
        let new_str = ~"crumpet.html";
        let new_url = make_url(move new_str, Some(move old_url));
        assert new_url.scheme == ~"http";
        assert new_url.host == ~"example.com";
        assert new_url.path == ~"/snarf/crumpet.html";
    }

}

pub type UrlMap<T: Copy> = HashMap<Url, T>;

pub fn url_map<T: Copy>() -> UrlMap<T> {
    use core::to_str::ToStr;

    HashMap::<Url, T>()
}
