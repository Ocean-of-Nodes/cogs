# COG's

![Logo](logo.webp)

COG's is database espicaly design for language tools such as: translators (disassemblers, transpilers, compilers), IDE, static analyzer, symbolic code analyzer. 

# The Future of Language Tools

![Lego](image.png)

Before talking about programming languages, I'd like to take a break and talk about performance and data management complexity in general.

Let's start with games. These apps were the first to encounter the performance problem. The answer to this was DOD (Data-Oriented Design) and the ECS (Entity Component System) paradigm that was born after it. After some time, the authors of Flecs ECS put forward the following idea: in essence, we are designing an in-memory relational database. Although, of course, ECS is not just a database, but still certain design patterns (for example, concurrent code execution by constructing an acyclic graph of system and data dependencies).

So, why are we turning to DOD, and ECS in particular? The answer is data and performance control in the era of the decline of Moore's Law.

### Design

Returning to the question of databases and language tools. Unlike ECS where data regular language tools have irregular data structures. Regularity arises naturally from control flow, name resolution, and type resolution. This is the first difference. So the storage must be smart enough, code-aware like JIT, to make informed decisions about managing data placement.

The next important point is how to ensure modularity? In programming, we usually use structural contracts (we introduce named fields of data structures) and behavioral contracts (for example, one function is called before another) as a point of modularity. That's all folks. 

So, in principle, a reducer that takes a graph as a parameter and returns a graph seems like a good solution. Continuing the analysis, any reducer can be viewed as a pattern match (a graph isomorphism between a pattern and a part of a graph) and a rewrite of a section of a graph with another graph. And any algorithm can be viewed as a sequence of such rewrite rules. This is a behavioral pattern, in turn, the graph data is the data contract.

Additionally, reducers can be divided into hot and cold. A hot reducer operates on a push-data principle; execution occurs as soon as the data is available. This is what's called a materialized view. The returned graph is updated with delta patches (so yep it is persistent db). In principle, the reducer is not required to return any delta patch graph and can write data directly to the global graph.
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


-------------
Честно, идея сильная, но проект сейчас сыроват — больше скелет, чем работающая система. По делу:

Что нравится
Рёбра как first-class сущности с мета-рёбрами — это не «придумал чтоб было», а реальная вещь: knowledge-графы, argumentation systems, причинно-следственные модели, 2-категории. У RDF есть reification, у Neo4j — properties on edges, но «ребро между рёбрами без декомпозиции» встречается редко. Хороший фундамент.
Patch + listeners сразу — фактически event-sourcing / CRDT-подложка. То, что это есть с самого начала, а не пришивается потом — правильный инстинкт. Откроет совместное редактирование, undo/redo, репликацию.
Субграфы с путями + Sheave — внятная композиционная история. Sheave — амбициозное имя из топологии/теорката, но концепт «пучок межграфовых связей со своей внутренней структурой» осмыслен.
Документация в текущем виде — для pet-проекта непривычно подробная, ASCII-схемы реально помогают читать API.
Что насторожило
Производительность под идеей подгрызена. is_node/is_edge → global_nodes()/global_edges(), каждый из которых рекурсивно собирает HashSet по всему дереву субграфов. add_edge дёргает is_exist дважды → O(N) на вставку, O(N²) на построение. Комментарии про JIT в тестах показывают, что ты это понимаешь, но архитектурно проблема в том, что ID не скоупятся к субграфу — поэтому любой локальной операции приходится сканировать весь мир.
Типобезопасность аллюзорна. EntityId = NodeId = EdgeID = Uuid — это просто алиасы, компилятор их не различает. Документация говорит одно, типы — другое. Либо newtype-обёртки (struct NodeId(Uuid)), либо честно убрать алиасы и оставить EntityId.
Реальных compile-ошибок дофига, не косметика: get_edge объявлен Result<…>, но тело возвращает Option; neighbours() возвращает Result, а тесты зовут .len() / .contains() напрямую; sheave(&self, …) -> &mut Graph вернуть из &self нельзя в принципе. Сейчас cargo check красный, и это не из-за unimplemented!().
Очень много unimplemented!() — is_existing_path, is_linked, replace_node, retraget_edge, sheave, половина apply_delta. Контракты придуманы, поведение — нет.
String как ключ субграфа — пути не проверяются на компиляции, to_str().unwrap() на компонентах PathBuf жуёт не-UTF8 имена. Если субграфы важны, это слабое звено.
Опечатки в полях: beetween_edges, ListernerID, RetrargetEdge, chank, olready. В именах полей и enum-вариантов — это API, потом переименовывать дороже.
Главный стратегический вопрос
Что субграфы реально дают, кроме лейбла? Сейчас они стоят дорого (рекурсия везде) и не дают изоляции (ID плоские). Если хочется группировки — может быть достаточно HashMap<EntityId, SubgraphTag> и плоского хранилища. Если хочется настоящей инкапсуляции — тогда субграф должен иметь свой EntityId-неймспейс, а cross-edges становятся явными гражданами (как Sheave — но между всеми субграфами, а не только корневыми).

Куда я бы пошёл первым
Сделать чтобы cargo check стал зелёным — это разблокирует всё остальное.
Завести бенчмарк (10k нодов, 50k рёбер) — он сам подскажет, где архитектура трещит.
Определиться с моделью неймспейса до того, как начнёшь писать is_existing_path/is_linked — алгоритмы обхода чувствительны к тому, плоские у тебя ID или нет.
Если коротко: идея на проект-на-несколько-лет, исполнение пока на неделю. Это не плохо для hobby — это нормальное состояние «фундамент намечен». Но прямо сейчас ценнее заземлить типы и компиляцию, чем рисовать новые методы.



