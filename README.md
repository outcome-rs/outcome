<p align="center">
	<img src=".github/github-logo.png" alt="outcome github logo" />
</p>
<br>
<p align="center">
  <a href="#">
    <img alt="version" src="https://img.shields.io/badge/version-0.1.0-yellow" />
  </a>
  <a href="#">
    <img alt="documentation" src="https://img.shields.io/badge/read%20the-docs-blue" />
  </a>
  <a href="https://book.theoutcomeproject.com">
    <img alt="documentation" src="https://img.shields.io/badge/read%20the-book-9cf" />
  </a>
  <a href="https://theoutcomeproject.com">
    <img alt="website" src="https://img.shields.io/badge/project-website-yellowgreen" />
  </a>
  <a href="https://discord.com/invite/VxC4ssK7eX">
    <img alt="discord" src="https://img.shields.io/discord/540935926541713419?label=discord%20chat" />
  </a>
</p>
<p align="center">
  <a href="https://www.patreon.com/adamsky">
    <img alt="patreon" src="https://img.shields.io/badge/support%20on-patreon-orange" />
  </a>
  <a href="https://github.com/sponsors/adamsky">
    <img alt="patreon" src="https://img.shields.io/badge/support%20on-github-lightgray" />
  </a>
</p>

<br>

[`outcome`](https://github.com/outcome-rs/outcome) is an easy to use
distributed simulation engine for the modern age.

Assembling, running and interacting with complex discrete event simulations,
whether it's multiplayer game worlds, organic city simulations, or scientific
calculations, is still not a simple task. Especially when it comes to
distributing them across multiple machines in a dynamic way. 

Scalable, user-friendly distributed simulation remains a largely
unexplored territory, with only a handful of closed-source
[commercial](https://hadean.com/) [solutions](https://improbable.io/)
available. `outcome` is an attempt to change that.

*NOTE: Currently in an early stage of development.
Approach with caution.*


## Features

* **Fast Prototyping** - small changes to the model don't require recompiling
the whole system
* **Dynamic Scalability** - run locally or on multiple machines, scale
dynamically at runtime
* **Data Interoperability** - the engine deals with widely supported primitive
data types
* **Language-agnostic** - interaction with the engine is carried out by 
exchanging messages
* **Low Barrier to Entry** - runtime-level logic processor opens the modelling
process to non-programmers
* **Modularity** - models can be divided into smaller,
self-contained packages, making model development faster and model reuse less
cumbersome


## What can it do?

`outcome` provides a way for quickly assembling and running dynamically
scalable simulations. It remains largely generic, and as such can potentially
be used in many different domains, from games to social studies.

It has the ability to connect together multiple separate programs, each tasked
with simulating a different part, or layer, of the system, and make them
co-create a single un-sharded world. As such, it can be viewed as a high-level
coupling tool that's able to abstract away many difficult problems that come
with building distributed simulations.

As `outcome` focuses heavily on a networking interface, it can serve as
a one-stop-shop solution for creation of massively-multiplayer interactive
experiences. Here, in a simulation cluster made of multiple worker nodes, each
node can serve as a server for user clients to connect to, allowing for more
concurrent connections to the system as a whole.

It can be used as a sort of backend for applications that are based on
complex simulation models. For example deep-simulation strategy games can can
leverage `outcome` to speed up their development process, getting things like
modding support, ready-made sub-system models (e.g. climate or social models)
made by the community, multiplayer capability and distributed simulation out
of the box.


## How does it work?

`outcome` is based around the idea of bringing together different programs
and making them communicate in a coherent way. To make this happen, the engine
provides a unifying conceptual framework in the form of an entity-component
architecture.

Multiple different game engines, physics solvers, AIs and other programs
can seamlessly co-create a single large world together. They query and mutate
the distributed entity database provided by the engine using a simple messaging
system.

The entity-based approach makes it possible to quickly restructure the whole
system at runtime. The engine monitors network traffic and current load among
the workers, and re-distributes entities among them accordingly, minimizing
slow inter-machine queries. Easy dynamic addition and removal of worker nodes
for load-based scaling is another benefit to this approach.

The runtime provides synchronization mechanisms for clients, allowing for
running in lockstep as well as triggering activity based on custom-defined
events. Clients can either register sets of event-triggered queries to receive
a constant flow of needed data, or just ask for whatever they need, whenever
they need it.

Runtime-level logic processor is provided as an optional feature. Along with
simple scripting functionality, it allows for development of simulation models
without requiring the user to create external *service* programs that interact
with the engine. This processor, also called the *machine*, is directly tied to
the entity-component architecture used by the engine - when enabled, we get the
ability to associate each component with a single event-triggered state
machine.

As models are meant to be modular, notions of *scenarios* and *modules* are 
introduced. A *scenario*, existing as a collection of smaller packages called
*modules*, is the main abstraction over a runnable `outcome` model. At any
point, a `snapshot` of the current simulation state can be made and saved to
disk.

For more explanations regarding the inner workings of the engine 
[consult the book](
https://book.theoutcomeproject.com/content/concepts/basic-concepts.html).
If you're interested in learning more about actual implementation details
see individual crates' documentation.


## Installation

Either download a precompiled executable from the *releases* page or
execute the following in the command line (requires 
[`Rust`](https://rustup.rs/) to be installed on your system):

```
cargo install outcome-cli
```

You can also `git clone` and `cargo build --release` yourself. If you're not
sure what any of the above means, [see a more detailed instruction](
https://book.theoutcomeproject.com/content/getting-started/installation.html).


## Usage

To get a glimpse of what `outcome-cli` can do for you, type:

```
outcome --help
```

There are a few example scenarios provided with this repository. Try running
them with:

```
outcome run <path-to-scenario>
```

Alternatively, navigate to selected scenario's directory and just:

```
outcome run
```

By default, the `run` subcommand will start a local simulation instance and put
you in the *interactive mode*, letting you step through the simulation and
read out data in-between steps.

To learn more about creating, running and interacting with `outcome` simulations
[see relevant chapters from the book](
https://book.theoutcomeproject.com/content/getting-started/getting-started.html).
