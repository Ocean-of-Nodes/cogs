# The Future of Language Tools

![Lego](image.png)

Before talking about programming languages, I'd like to take a break and talk about performance and data management complexity in general.

Let's start with games. These apps were the first to encounter the performance problem. The answer to this was DOD (Data-Oriented Design) and the ECS (Entity Component System) paradigm that was born after it. After some time, the authors of Flecs ECS put forward the following idea: in essence, we are designing an in-memory relational database. Although, of course, ECS is not just a database, but still certain design patterns (for example, concurrent code execution by constructing an acyclic graph of system and data dependencies).

So, why are we turning to DOD, and ECS in particular? The answer is data and performance control in the era of the decline of Moore's Law.

### Design

Returning to the question of databases and language tools. Unlike ECS where data regular language tools have irregular data structures. Regularity arises naturally from control flow, name resolution, and type resolution. This is the first difference. So the storage must be smart enough, code-aware like JIT, to make informed decisions about managing data placement.

The next important point is how to ensure modularity? In programming, we usually use structural contracts (we introduce named fields of data structures) and behavioral contracts (for example, one function is called before another) as a point of modularity. That's all folks. 

So, in principle, a reducer that takes a graph as a parameter and returns a graph seems like a good solution. Continuing the analysis, any reducer can be viewed as a pattern match (a graph isomorphism between a pattern and a part of a graph) and a rewrite of a section of a graph with another graph. And any algorithm can be viewed as a sequence of such rewrite rules. This is a behavioral pattern, in turn, the graph data is the data contract.

Additionally, reducers can be divided into hot and cold. A hot reducer operates on a push-data principle; execution occurs as soon as the data is available. This is what's called a materialized view. The returned graph is updated with delta patches. In principle, the reducer is not required to return any delta patch graph and can write data directly to the global graph.
A cold reducer, also called a subroutine, returns a patch when manually called again.

Regarding reducers, the database here follows the logic of spacetimedb and stores the code compiled in wasm directly in the database itself.

Now a little about the query dialect. 

### What can be achieved with such a database?
- Using as a starting point as a single uniform representation for unioning rust compiler and analyzer
- As an external api for any kind of RA or compiler plugins

### Interesting opening opportunities
- User queries - user can write is own queries, they can replace regular text search or regular expression search 
- Now the language is present function like macros, they have some limitations as they work at the token level. We could pass to them on the direct handle of the compiler UGR engine. 
In this case, they would turn into something similar to compiler plugins, for example, they would have access to types, etc. It seems really like new level of metaprogramming but that requires discussion and elaboration.

### Areas that can be explored separately from the project
- Compilation as service - compilation happens continuously while you write code
- Various type of visualization
- Run partially valid code (not finished code) and step by step add pieces and continue execution
- Universal schema and convertor. [Dragon](https://eng.uber.com/dragon-schema-integration-at-uber-scale/) makes an effort in this direction, but we don't know, will Uber make the project public or not. This would be a major improvement for people who engaged in date mining or wont extract data in a special format



