create table item
(
    id         INTEGER not null
        constraint item_pk
            primary key,
    name       TEXT    not null,
    path       TEXT    not null,
    file_type  TEXT    not null,
    created_at TEXT default (STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')) not null,
    parent     INTEGER
        references item
            on update cascade on delete cascade,
    md5        TEXT    not null
);

create unique index item_id_uindex
    on item (id);

create unique index item_md5_uindex
    on item (md5);

create unique index item_path_uindex
    on item (path);

create table tag
(
    id         INTEGER not null
        primary key,
    name       TEXT    not null
        unique,
    created_at TEXT default (STRFTIME('%Y-%m-%d %H:%M:%f', 'NOW')) not null,
    alias      integer
);

create table item_tag
(
    id   INTEGER not null
        constraint item_tag_pk
            primary key,
    item INTEGER not null
        references item
            on delete cascade,
    tag  INTEGER not null
        references tag
            on delete cascade
);

create unique index item_tag_item_tag_uindex
    on item_tag (item, tag);

create unique index tag_id_uindex
    on tag (id);

create table tag_tag
(
    id  INTEGER not null
        constraint tag_tag_pk
            primary key,
    tag INTEGER not null
        references tag
            on delete cascade,
    dep INTEGER not null
        references tag
            on delete cascade
);

create unique index tag_tag_tag_dep_uindex
    on tag_tag (tag, dep);

