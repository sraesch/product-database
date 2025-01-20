create type QuantityType AS ENUM ('weight', 'volume');

-- The table that stores the product images like previews and full images
create table if not exists product_image (
    id SERIAL PRIMARY KEY, -- The id of the product image
    data bytea not null, -- The data of image
    content_type varchar(32) not null -- The content type of the image
);

-- The table stores the nutrition information of the products
-- All values are in grams or milligrams for the reference quantity of 100g
create table if not exists nutrients (
    id  SERIAL PRIMARY KEY, -- The id of the nutrients entry

    kcal real not null, -- The amount of kcal in the product

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

-- Products which have been scanned by the users, but are not in the database
create table if not exists reported_missing_products (
    id  SERIAL PRIMARY KEY, -- The id of the reported entry
    product_id varchar(64) not null, -- The id of the missing product
    date timestamp with time zone not null -- The date when the request was made
);

-- The table that stores full product descriptions
-- Product descriptions can be requested new products or regular products in the database
create table if not exists product_description (
    id SERIAL PRIMARY KEY, -- The id of the product info entry
    product_id varchar(64) not null, -- The id of the product
    name varchar(64) not null, -- The name of the product

    -- The quantity type is either weight or volume.
    -- Weight in grams is used for products like flour, sugar, etc.
    -- Volume in ml is used for products like milk, water, etc.
    quantity_type QuantityType not null,

    -- The amount for one portion of the product in grams or ml
    -- depending on the quantity type
    portion real not null,

    -- The ratio between volume and weight, i.e. volume(ml) = weight(g) * volume_weight_ratio
    -- Is only defined if the quantity type is volume
    volume_weight_ratio real,

    producer varchar(64), -- The producer of the product

    preview int,  -- Reference onto a preview image
    photo int, -- Reference onto a full image
    nutrients int, -- Reference onto the nutrients of the product

    FOREIGN KEY (preview) REFERENCES product_image(id) ON DELETE CASCADE,
    FOREIGN KEY (photo) REFERENCES product_image(id) ON DELETE CASCADE,
    FOREIGN KEY (nutrients) REFERENCES nutrients(id) ON DELETE CASCADE
);

-- The table that stores the products
create table if not exists products (
    product_id varchar(64) not null, -- The id of the product
    product_description_id int not null, -- The id of the product description entry

    PRIMARY KEY(product_id),
    FOREIGN KEY (product_description_id) REFERENCES product_description(id) ON DELETE CASCADE
);

-- This table stores requested products
create table if not exists requested_products(
    id  SERIAL PRIMARY KEY, -- The id of the entry
    product_description_id int not null, -- The id of the product description entry
    date timestamp with time zone  not null, -- The date when the product was missing
    FOREIGN KEY (product_description_id) REFERENCES product_description(id) ON DELETE CASCADE
);
