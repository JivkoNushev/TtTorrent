# Refactoring:
- *String vs &str*

# TODO:
- enum for the messages passed through the channels
- spawn_blocking for the parsing

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
