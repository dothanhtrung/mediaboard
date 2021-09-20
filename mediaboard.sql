-- we don't know how to generate root <with-no-name> (class Root) :(
create table item
(
    id INTEGER
        constraint item_pk
            primary key,
    name TEXT,
    path TEXT,
    file_type TEXT,
    created_at TEXT default (STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')) not null,
    parent INTEGER
        references item
            on update cascade on delete cascade,
    md5 TEXT
);

create unique index item_id_uindex
    on item (id);

create unique index item_md5_uindex
    on item (md5);

create unique index item_path_uindex
    on item (path);

create table tag
(
    id INTEGER
        primary key,
    name TEXT
        unique,
    created_at TEXT default (STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')) not null
);

create table item_tag
(
    id INTEGER
        constraint item_tag_pk
            primary key,
    item INTEGER
        references item
            on delete cascade,
    tag INTEGER
        references tag
            on delete cascade
);

create unique index item_tag_item_tag_uindex
    on item_tag (item, tag);

create unique index tag_id_uindex
    on tag (id);

create table tag_tag
(
    id INTEGER
        constraint tag_tag_pk
            primary key,
    tag INTEGER
        references tag
            on delete cascade,
    dep INTEGER
        references tag
            on delete cascade
);

create unique index tag_tag_tag_dep_uindex
    on tag_tag (tag, dep);

