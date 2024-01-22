# Refactoring:
- *String vs &str*

# TODO:
- spawn_blocking for the parsing maybe

- use select for catching messages
```
loop {
  select! {
    Some(MessageEnum) = rx.recv() => ...,
    Some(MessageEnum) = rx2.recv() => ...
  }
}
```

- can use std::thread for the cpu intensive tasks like parsing
- use Streams when iterating
- use tracing crate for logging

- use repr(C)

- fix peer tcp connections

- there are functions that return options but only return values or panic

- is 
let mut message = serde_json::to_string(&message).context("Failed to serialize message")?;
message.push('\n');

better than

let message = serde_json::to_string(&message).context("Failed to serialize message")?;
message = message + '\n';