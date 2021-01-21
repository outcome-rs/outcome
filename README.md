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
  <a href="https://discord.gg/tB6uy6f">
    <img alt="discord" src="https://img.shields.io/discord/540935926541713419?label=discord%20chat" />
  </a>
</p>
<p align="center">
  <a href="https://patreon.com/theoutcome">
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
calculations, is still not an easy task. Especially when it comes to
distributing them across multiple machines in a dynamic way. Aside from a few
existing [commercial](https://hadean.com/) [solutions](https://improbable.io/),
scalable user-friendly distributed simulation still remains a mostly unexplored
territory. This project is an attempt to change that.

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

`outcome` provides a generic interface for quickly assembling dynamically
scalable simulations. As such it has potential uses in many different domains.

It has the ability to connect together many separate programs, each tasked with
simulating a different part of the system, and make them co-create a single
unsharded world. This means that new models can be assembled from already existing,
battle-tested modules.


One crucial aspect of this project is it's focus on the ease of use. 



## How does it work?

`outcome` is based around the idea of bringing together different programs
and making them communicate in a coherent way. To make this happen, the engine
provides a unifying conceptual framework in the form of an entity-component
architecture.

Multiple different game engines, physics solvers and agent-based AI models,
can simulate a single large world without having to keep track of entities
distributed around a collection of machines.

Engine runtime does dynamic optimisation of the system by monitoring current
load and network traffic and transferring entities across machines accordingly.

:



This entity-focused approach makes it possible to quickly restructure the whole
system at runtime. We can leverage this by constantly re-arranging entities
in a way that minimizes inter-machine queries. It also makes dynamically adding
and removing worker nodes for load-based scaling feasible.

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
