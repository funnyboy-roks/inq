# inq\[uire\]

A tool for executing API queries defined in a plaintext file that can be
checked in to version control or managed like any other file.

The basic workflow goes like this:

1. Create the configuration file with `variables` and `queries`
   sections
1. Populate the `variables` with values that you wish to use throughout
   the queries
1. Specify queries and the details that should be use for the requests
1. Run any query using `inq query <query>`

## Configuration

The configuration uses the [kdl](https://kdl.dev) format.

### Variables

Variables are the way to have central values and customise behaviour at
runtime.  All variables may be overridden with `--var KEY=VALUE` on the
commandline.

Most strings support interpolation using [rhai] with the syntax
`${expression}`.  Each variable in the `variables` block is defined in
the scope and may be used as a value in the expression.  (NOTE: all
variables are strings and my be converted into other values using the
rhai functions).

Variables can be specified in three different ways:

- `foo <value>` - Use a specific value
- `env=<variable>` - Always read the value from an environment variable
- `file=<file>` - Use the contents of a file as the variable (whitespace trimmed)

Variables may additionally be marked as `persist`.  These variables
will be saved to a file and hold their values between queries.

To mark a variable to persist, add `persist=#true` or `persist="<time>"`
where `<time>` is a duration, like `1 hour`.

These go in the `variables` section:

```kdl
variables {
    PORT 3000
    BASE_URL "http://localhost:${PORT}"
    USER env="USERNAME"
    PASSWORD file="password.txt"
    COOKIE "" persist=#true
}
```

#### Persistent Variables

Variables that have been marked as `persist` can be updated using the
`inq variable` subcommand:

```
inq var[iable] set <variable> [value] [--expires=<time>]
inq var[iable] get <variable>
inq var[iable] list
```

### Queries

Queries are the basic requests, any number of which may be specified in
the `queries` section.

Each query is specified using

```kdl
<NAME> <METHOD> <URL> {
    // configuration
}
```

The configuration may contain the following items:

- `headers` - a map of string to string that will be used to set the
  request headers
- `body` - Set the body of the request using either `text=<string>` or
  `json=<string>`.  If `json` is used, the `content-type` header is
  automatically set to `application/json`.
- `post-script` - A [rhai] script to run after the request is complete.
  This can be used for updating variables


## Example

```kdl
variables {
    PORT 3000
    BASE_URL "http://localhost:${PORT}"
    USER env="USERNAME"
    PASSWORD file="password.txt"
    COOKIE "" persist=#true
}

queries {
    login POST "${BASE_URL}/login" {
        headers {
            user-agent "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:50.0) Gecko/20100101 Firefox/50.0"
        }
        body json="""
        {
            "username": "${USER}",
            "password": "${PASSWORD"}
        }
        """
        post-script #"""
        let cookie = parse_cookie(response.headers["set-cookie"]);
        vars.COOKIE.value = cookie.value;
        vars.COOKIE.expires_at = cookie.expires;
        """#
    }
}
```

And then run it with

```sh
inq query login
# override variables with
inq query login --var USER=someone-else
```

[rhai]: https://rhai.rs/book/language/comments.html
