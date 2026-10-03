# inq\[uire\]

A tool for executing API queries defined in a plaintext file that can be
checked in to version control or managed like any other file.

The basic workflow goes like this:

1. Create the script
1. Run any route using `inq route <route>`

## Install

There are a few ways to install:

- Download binary from the [releases](https://github.com/funnyboy-roks/inq/releases/latest)
- Use [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):
  ```
  cargo binstall inq
  ```
- Use cargo install:
  ```
  cargo install inq
  ```

## Configuration

The configuration uses a custom language called inq (crazy)

### Variables

Variables in inq are lazy.  The value assigned to them will not be
evaluated until they are used by anything other than another variable.

Just like in most programming languages, variables can exist in two
ways: Global and Local.  Global variables are defined at the top-level
of the file and can be accessed from any scope within.  Local variables
are limited to their scope and their scope's children.

Variables are defined using the `let` keyword, like so:

```inq
let my_variable = 5 + 3;
```

#### Special Variables

Some global variables are special and affect the inq differently.

Currently, the only special variable is `BASE_URL` and will be applied
to all routes that don't have a scheme set.

```inq
let BASE_URL = "https://example.com"; 
```

#### Persistent Variables

Global variables may have the `#[persist]` annotation applied which will
make their value persistent across calls.  This is intended to be used
for things like authentication tokens.

```inq
#[persist]
let cookie = "";

// in an after block:
cookie = response.headers["cookie"];
```

<!-- TODO:
```
inq var[iable] set <variable> [value] [--expires=<time>]
inq var[iable] get <variable>
inq var[iable] list
```
-->

### Routes

Routes are individual routes that may be called from the command-line.

A route can be specified like so:

```inq
route <name>(<args>) => <method> <url>;
// or
route <name>(<args>) => <method> <url>
    before { // optional
        request.body = "hello world";
    }
    after { // optional
        print(response.headers["set-cookie"])
    }
```

## Example

Here is a complete example of an inq script:

```inq
let port = 3000;
let BASE_URL = http://localhost:$port;
let user = env("USERNAME");
let password = read_text_file("password.txt");
#[persist]
let cookie = cookie;

route login => POST "${base_url}/login"
    before {
        request.headers["user-agent"] = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:50.0) Gecko/20100101 Firefox/50.0";
        request.body = json({
            username: user,
            password: password,
        })
    }
    after {
        let cookie_header = parse_cookie(response.headers["set-cookie"]);
        cookie = cookie_header.value;
    }
```

And then run it with

```sh
inq route login
```
