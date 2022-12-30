# gRPC-Web Hello

Out of masochistic curiosity, I decided to make an end-to-end gRPC-Web setup
with a backend server in Rust, deployable as a standalone Docker image. I was
curious to see how the development experience of gRPC with server-side streaming
compares to WebSockets, the latter of which has never been a favorite API of
mine.

My goal was to build a simple toy program where when you open the page in your
browser, it subscribes you to a stream of updates that tells you how many total
subscriptions there are and prints that number on the page. So if you were to
open the page in multiple tabs, the number should change in all tabs to be the
number of currently open tabs, and it should go back down in all tabs as tabs
are closed.

I'm kind of bad at computers, so this took me a lot longer than I was hoping.
Here are some of the things that went wrong.

## Frontend code

- At this moment, the latest version of the Protobuf compiler doesn't work for
  generating JS code! It's been like this for a full month, which is kind of
  mind-blowing. See the [related GitHub
  issue](https://github.com/protocolbuffers/protobuf-javascript/issues/127).

  - To solve this, I thought I'd put in some legwork and build the protobuf-js
    plugin from source as described in [@johejo's
    instructions](https://github.com/protocolbuffers/protobuf-javascript/issues/127#issuecomment-1204202870).
    The build failed when I ran it on my Mac (and I went through all the trouble
    of installing Bazel too). Next I tried doing the build in a Debian
    container, which succeeded, but then the executable wouldn't work on my Mac
    outside the container. Rather than putting more time into this, I just
    downgraded protoc as described [by @clehene
    here](https://github.com/protocolbuffers/protobuf-javascript/issues/127#issuecomment-1204202844).

- Generated JS protobuf files greatly increase the frontend bundle size, adding
  230 KB parsed and 46 KB gzipped. This is because the generated files depend on
  `google-protobuf`, a gigantic dependency seemingly not subject to treeshaking
  or dead-code elimination. This is very shitty, and at the moment it seems that
  anyone using gRPC-Web just has to live with it.

- When you want to use server-side streaming, you need to put your generated
  gRPC stuff into "text mode", meaning it sends all its protobuf messages as
  base-64 strings rather than binary data. This means that many messages are
  actually larger than equivalent JSON. Hopefully the size increase is negated
  by compression, but it still feels a little wasteful.

## Vanilla gRPC with Rust

My next goal was to build a normal Rust gRPC server (not gRPC-Web) based on
[Tonic](https://github.com/hyperium/tonic), then run it out of a container.

- At first, this worked fine locally but failed in the container. As it turned
  out, the issue was this line, copied from an example:

  ```rust
  let addr = "[::1]:50051".parse()?;
  ```

  For reasons I don't understand, to get this to work out of a Docker container
  and not just locally, I needed to change it to

  ```rust
  let addr = "[::]:50051".parse()?;
  ```

- The only other tricky part of the Rust implementation was detecting when the
  client has cancelled out of a server-side stream. The way Tonic indicates this
  is that the `Stream` that our method returns is dropped, so we need to provide
  a custom `Stream` implementation that has a custom implementation of the
  `Drop` trait. This conceptually makes sense (it means the program will never
  end up hanging on to a `Stream` instance after the client has left), but felt
  a bit clunky to actually work with. I could probably make this better by
  writing some helpers.

- I'm glad I did this custom `Stream` implementation because it requires this
  method to be implemented:

  ```rust
  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>
  ```

  which forced me actually learn what the `Pin` trait means. I've tried to learn
  this before but [the docs](https://doc.rust-lang.org/std/pin/index.html)
  always scared me off. For a while I thought I would have to implement what the
  docs call [structural
  pinning](https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field)
  which is a little worrying because it uses `unsafe` code, but as it turns out
  I didn't need to get into that at all.

- I spent a bit of time trying to get the build to work on Alpine just for the
  challenge. I couldn't get it to work and I think it's not worth the trouble. I
  can afford my images being a few tens of MB larger to not have to deal with
  musl, plus it'll get much worse later anyways because of Envoy as we'll see in
  the next section.

## To gRPC-Web

Now to turn gRPC into gRPC-Web. This means we need to run a reverse-proxy in
front of our gRPC server from the previous section. The proxy will do something
involving turning the browser's HTTP/1.1 into the HTTP/2 our gRPC server
understands.

I wasted a **lot** of time here trying to do this with Envoy first, because
that's the only proxy mentioned in the docs on the [gRPC
website](https://grpc.io/docs/platforms/web/). Let's see how that went.

### Envoy

- Running this locally at first wasn't bad. I spun up an Envoy container and
  gave it the example configuration from the gRPC-Web docs. The only thing I
  needed change from the example was

  ```yaml
  socket_address:
    address: 0.0.0.0
    port_value: 9090
  ```

  to

  ```yaml
  socket_address:
    address: host.docker.internal
    port_value: 50051
  ```

  since we want the container to forward to the locally running Rust server on
  my host machine (and at a different port than the example).

- One extremely frustrating bug here: while the example named the config file
  `envoy.yaml`, I was naming it `envoy.yml`. As it turns out, that doesn't work,
  even when you manually specify the config file. Envoy doesn't recognize the
  `.yml` extension and tries to parse it as JSON! Wtf?

- Now to make a standalone Docker image. I figured I'd try to pack Envoy and the
  server into the same image, because it would be easy to deploy and configure.
  Probably experienced devops engineers would say to have them separate so they
  could scale differently or something, but I feel like this would make
  deployment significantly more complicated if I just want to deploy a single
  server. Maybe Kubernetes makes this easy, I dunno.

- At first, I wanted to build the server in a `rust` image, then copy the
  executable into the `envoyproxy/envoy` image. When I tried this, the server
  wouldn't run in the Envoy image because of not having the right version of
  glibc. I figured I had the choice of either installing Rust into the Envoy
  image so I could build a compatible executable, or installing Envoy into a a
  different image that was capable of running the executable. I went with the
  latter, because I feel like my Rust server, the point of the whole exercise,
  shouldn't be beholden to environmental oddities on the Envoy image.

- So, I decided to use a `debian/buster-slim` image, copy the Rust executable
  into it, and install Envoy into it via the [somewhat lengthy set of
  commands](https://www.envoyproxy.io/docs/envoy/latest/start/install#install-envoy-on-debian-gnu-linux)
  from their Getting Started guide.

- The `debian:buster-slim` base image is 69 MB. My Rust gRPC server executable
  is only 7 MB (!!). But adding Envoy to the image adds 200 MB. This puts the
  total image size at 276 MB, the vast majority of which is Envoy and its
  dependencies. Hope all that space helps it do its job well!

### gRPC Web Proxy

Eventually I noticed that the gRPC-Web GitHub readme also mentioned another
proxy, [gRPC Web
Proxy](https://github.com/improbable-eng/grpc-web/tree/master/go/grpcwebproxy).
I should have just used this in the beginning. Its big advantages:

- Much simpler configuration.
- It's 15 MB instead of 200 MB.
- It has really nice gRPC-specific logging.
- It has a standalone executable, so it's easy to add to containers.
- The docs actually say it's intended to be used as a companion process in a
  gRPC server container, so I don't feel guilty about putting the proxy and
  server in the same container anymore.

I replaced Envoy and got this working in a tenth of the time.

### tonic-web

As it turns out, I didn't need to do any of this because Rust's gRPC library
supports gRPC-Web on its own, if you use the `tonic_web` crate. There's an
undocumented need to set up CORS when you do this, which I found buried in a
[GitHub
issue](https://github.com/hyperium/tonic/issues/1174#issuecomment-1332341548),
but once you do that this just works and we no longer need to run other
processes.

I'm still glad I did it the other way first, because you would need to do this
if your server were in a language which doesn't have a gRPC-Web library.

## Conclusion

Was it worth it? The alternative to gRPC-Web would be to implement the
subscriptions with WebSockets instead. What are the pros and cons?

### gRPC-Web advantages

- Generated stubs. Kind of significant because implementing a WebSocket
  interface from scratch is hard work, both on the client- and server-sides.
- If the server has clients on both the frontend and backend, then all clients
  can use the same API rather than having a separate APIs for frontend and
  backend.

### gRPC-Web disadvantages

- Huge frontend bundle size increase (230 KB parsed, 46 KB gzipped).
- Can't reasonably use the Network tab to debug payloads.
- Unclear if it provides any benefits in speed or bandwidth over JSON, due to
  using base-64 text rather than binary.
- As of today (August 8), the latest version of protoc is completely broken for
  JavaScript, which lowers my faith in how well this is supported.

The first drawback is the worst one and it's terrible. And yet, in spite of it
I'm still tempted to keep using gRPC-Web purely to avoid writing WebSocket
communications by hand, with all of their error-checking and state management in
both the client and server.
