create type IDType AS ENUM ('barcode', 'qr', 'uuid');

create table if not exists products (
    product_id varchar(64) not null, -- The id of the product
    name varchar(255) not null, -- The name of the product
    producer varchar(255), -- The producer of the product
    preview bytea, -- Small version of a product image
    preview_content_type varchar(32), -- The content type of the preview image
    photo bytea, -- Full version of a product image

    protein_grams real, -- The amount of protein in the product in grams
    fat_grams real, -- The amount of fat in the product in grams
    carbohydrates_grams real, -- The amount of carbohydrates in the product in grams
    sugar_grams real, -- The amount of sugar in the product in grams
    salt_grams real, -- The amount of salt in the product in grams
    vitaminA_mg real, -- The amount of vitamin A in the product in milligrams
    vitaminC_mg real, -- The amount of vitamin C in the product in milligrams
    vitaminD_Mg real, -- The amount of vitamin D in the product in micrograms
    iron_mg real, -- The amount of iron in the product in milligrams
    calcium_mg real, -- The amount of calcium in the product in milligrams
    magnesium_mg real, -- The amount of magnesium in the product in milligrams
    sodium_mg real, -- The amount of sodium in the product in milligrams
    zinc_mg real, -- The amount of zinc in the product in milligrams

    PRIMARY KEY(product_id)
);

create table if not exists reported_missing_products (
    -- Note: We cannot use the product key as primary key as it might occur multiple times.
    -- Multiple users might have reported the same missing product.
    -- Therefore, we use a dedicated id.
    id  SERIAL PRIMARY KEY, -- The id of the entry
    product_id varchar(64) not null, -- The id of the missing product
    date timestamp with time zone not null -- The date when the product was missing
);

create table if not exists requested_products(
    -- Note: We cannot use the product key as primary key as it might occur multiple times.
    -- Multiple users might have requested the same product.
    -- Therefore, we use a dedicated id.
    id  SERIAL PRIMARY KEY, -- The id of the entry
    product_id varchar(64) not null, -- The id of the missing product
    date timestamp with time zone  not null, -- The date when the product was missing

    name varchar(255) not null, -- The name of the product
    producer varchar(255), -- The producer of the product
    preview bytea, -- Small version of a product image
    preview_content_type varchar(32), -- The content type of the preview image
    photo bytea, -- Full version of a product image

    protein_grams real, -- The amount of protein in the product in grams
    fat_grams real, -- The amount of fat in the product in grams
    carbohydrates_grams real, -- The amount of carbohydrates in the product in grams
    sugar_grams real, -- The amount of sugar in the product in grams
    salt_grams real, -- The amount of salt in the product in grams
    vitaminA_mg real, -- The amount of vitamin A in the product in milligrams
    vitaminC_mg real, -- The amount of vitamin C in the product in milligrams
    vitaminD_Mg real, -- The amount of vitamin D in the product in micrograms
    iron_mg real, -- The amount of iron in the product in milligrams
    calcium_mg real, -- The amount of calcium in the product in milligrams
    magnesium_mg real, -- The amount of magnesium in the product in milligrams
    sodium_mg real, -- The amount of sodium in the product in milligrams
    zinc_mg real -- The amount of zinc in the product in milligrams
);