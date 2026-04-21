# inq

A command-line API client that allows for saving requests

## Example

The configuration uses [kdl](https://kdl.dev)

```kdl
variables {
    PORT 3000
    BASE_URL "http://localhost:${PORT}"
    USER "john"
    PASSWORD "my_password"
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
    }
}
```

And then run it with

```sh
inq login
# override variables with
inq login --var USER=someone-else
```
