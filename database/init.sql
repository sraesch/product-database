-- Activate the pg_trgm extension for fuzzy search
CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

--
-- DEFINITION OF TABLES, TYPES AND INDICES
--
-- Define type for the quantity type of the product
CREATE TYPE QuantityType AS ENUM(
    'weight',
    'volume'
);

-- The table that stores the product images like previews and full images
CREATE TABLE IF NOT EXISTS product_image(
    id serial PRIMARY KEY, -- The id of the product image
    data bytea NOT NULL, -- The data of image
    content_type varchar(32) NOT NULL -- The content type of the image
);

-- The table stores the nutrition information of the products
-- All values are in grams relative to the reference quantity of 100g
CREATE TABLE IF NOT EXISTS nutrients(
    id serial PRIMARY KEY, -- The id of the nutrients entry
    kcal real NOT NULL, -- The amount of kcal in the product
    protein_grams real, -- The amount of protein in the product in grams
    fat_grams real, -- The amount of fat in the product in grams
    carbohydrates_grams real, -- The amount of carbohydrates in the product in grams
    sugar_grams real, -- The amount of sugar in the product in grams
    salt_grams real, -- The amount of salt in the product in grams
    vitamin_a_mg real, -- The amount of vitamin A in the product in milligrams
    vitamin_c_mg real, -- The amount of vitamin C in the product in milligrams
    vitamin_d_mug real, -- The amount of vitamin D in the product in micrograms
    iron_mg real, -- The amount of iron in the product in milligrams
    calcium_mg real, -- The amount of calcium in the product in milligrams
    magnesium_mg real, -- The amount of magnesium in the product in milligrams
    sodium_mg real, -- The amount of sodium in the product in milligrams
    zinc_mg real -- The amount of zinc in the product in milligrams
);

-- Products which have been scanned by the users, but are not in the database
CREATE TABLE IF NOT EXISTS reported_missing_products(
    id serial PRIMARY KEY, -- The id of the reported entry
    product_id varchar(64) NOT NULL, -- The id of the missing product
    date timestamp with time zone NOT NULL -- The date when the request was made
);

-- Index for product_id in reported_missing_products
CREATE INDEX IF NOT EXISTS reported_missing_products_product_id_index ON reported_missing_products(product_id);

-- The table that stores full product descriptions
-- Product descriptions can be requested new products or regular products in the database
CREATE TABLE IF NOT EXISTS product_description(
    id serial PRIMARY KEY, -- The id of the product info entry
    product_id varchar(64) NOT NULL, -- The id of the product
    name varchar(64) NOT NULL, -- The name of the product
    producer varchar(64), -- The producer of the product
    -- The quantity type is either weight or volume.
    -- Weight in grams is used for products like flour, sugar, etc.
    -- Volume in ml is used for products like milk, water, etc.
    quantity_type QuantityType NOT NULL,
    -- The amount for one portion of the product in grams or ml
    -- depending on the quantity type
    portion real NOT NULL,
    -- The ratio between volume and weight, i.e. volume(ml) = weight(g) * volume_weight_ratio
    -- Is only defined if the quantity type is volume
    volume_weight_ratio real,
    preview int, -- Reference onto a preview image
    photo int, -- Reference onto a full image
    nutrients int NOT NULL, -- Reference onto the nutrients of the product
    FOREIGN KEY (preview) REFERENCES product_image(id) ON DELETE CASCADE,
    FOREIGN KEY (photo) REFERENCES product_image(id) ON DELETE CASCADE,
    FOREIGN KEY (nutrients) REFERENCES nutrients(id) ON DELETE CASCADE
);

-- Index for product_id in product_description
CREATE INDEX IF NOT EXISTS product_description_product_id_index ON product_description(product_id);

-- Index for the name of the product in product_description
CREATE INDEX IF NOT EXISTS product_description_name_trgm_idx ON product_description USING gist(name gist_trgm_ops);

-- The table that stores the products
CREATE TABLE IF NOT EXISTS products(
    product_id varchar(64) NOT NULL, -- The id of the product
    product_description_id int NOT NULL, -- The id of the product description entry
    PRIMARY KEY (product_id),
    FOREIGN KEY (product_description_id) REFERENCES product_description(id) ON DELETE CASCADE
);

-- This table stores requested products
CREATE TABLE IF NOT EXISTS requested_products(
    id serial PRIMARY KEY, -- The id of the entry
    product_description_id int NOT NULL, -- The id of the product description entry
    date timestamp with time zone NOT NULL, -- The date when the product was missing
    FOREIGN KEY (product_description_id) REFERENCES product_description(id) ON DELETE CASCADE
);

--
-- DEFINITION OF VIEWS
--
-- Create a view that joins the requested products with the product description and nutrients
CREATE VIEW requested_products_full AS
SELECT
    r.id r_id,
    r.date,
    p.name,
    p.producer,
    p.quantity_type,
    p.portion,
    p.product_id,
    p.volume_weight_ratio,
    p.preview,
    p.photo,
    n.kcal,
    n.protein_grams,
    n.fat_grams,
    n.carbohydrates_grams,
    n.sugar_grams,
    n.salt_grams,
    n.vitamin_a_mg,
    n.vitamin_c_mg,
    n.vitamin_d_mug,
    n.iron_mg,
    n.calcium_mg,
    n.magnesium_mg,
    n.sodium_mg,
    n.zinc_mg
FROM
    requested_products r
    JOIN product_description p ON p.id = r.product_description_id
    JOIN nutrients n ON p.nutrients = n.id;

-- Create a view that joins the requested products with the product description and nutrients including the preview image
CREATE VIEW requested_products_full_with_preview AS
SELECT
    r.id AS r_id,
    r.date,
    p.name,
    p.producer,
    p.quantity_type,
    p.portion,
    p.product_id,
    p.volume_weight_ratio,
    pi.data AS preview,
    pi.content_type AS preview_content_type,
    p.photo,
    n.kcal,
    n.protein_grams,
    n.fat_grams,
    n.carbohydrates_grams,
    n.sugar_grams,
    n.salt_grams,
    n.vitamin_a_mg,
    n.vitamin_c_mg,
    n.vitamin_d_mug,
    n.iron_mg,
    n.calcium_mg,
    n.magnesium_mg,
    n.sodium_mg,
    n.zinc_mg
FROM
    requested_products r
    JOIN product_description p ON p.id = r.product_description_id
    JOIN nutrients n ON p.nutrients = n.id
    LEFT JOIN product_image pi ON p.preview = pi.id;

-- Create a view that joins the products with the product description and nutrients
CREATE VIEW products_full AS
SELECT
    r.product_id,
    p.name,
    p.producer,
    p.quantity_type,
    p.portion,
    p.volume_weight_ratio,
    p.preview,
    p.photo,
    n.kcal,
    n.protein_grams,
    n.fat_grams,
    n.carbohydrates_grams,
    n.sugar_grams,
    n.salt_grams,
    n.vitamin_a_mg,
    n.vitamin_c_mg,
    n.vitamin_d_mug,
    n.iron_mg,
    n.calcium_mg,
    n.magnesium_mg,
    n.sodium_mg,
    n.zinc_mg
FROM
    products r
    JOIN product_description p ON p.id = r.product_description_id
    JOIN nutrients n ON p.nutrients = n.id;

-- Create a view that joins the products with the product description and nutrients including the preview image
CREATE VIEW products_full_with_preview AS
SELECT
    r.product_id,
    p.name,
    p.producer,
    p.quantity_type,
    p.portion,
    p.volume_weight_ratio,
    pi.data AS preview,
    pi.content_type AS preview_content_type,
    p.photo,
    n.kcal,
    n.protein_grams,
    n.fat_grams,
    n.carbohydrates_grams,
    n.sugar_grams,
    n.salt_grams,
    n.vitamin_a_mg,
    n.vitamin_c_mg,
    n.vitamin_d_mug,
    n.iron_mg,
    n.calcium_mg,
    n.magnesium_mg,
    n.sodium_mg,
    n.zinc_mg
FROM
    products r
    JOIN product_description p ON p.id = r.product_description_id
    JOIN nutrients n ON p.nutrients = n.id
    LEFT JOIN product_image pi ON p.preview = pi.id;

-- View on full images for the product requests
CREATE VIEW requested_products_full_image AS
SELECT
    r.id AS r_id,
    pi.data,
    pi.content_type
FROM
    requested_products r
    JOIN product_description p ON p.id = r.product_description_id
    JOIN product_image pi ON p.photo = pi.id;

--
-- DEFINITION OF FUNCTIONS
--
-- Trigger function to delete the product description when a product request is deleted
CREATE OR REPLACE FUNCTION trigger_func_delete_product_or_requested_product()
    RETURNS TRIGGER
    AS $$
BEGIN
    DELETE FROM product_description
    WHERE id = OLD.product_description_id;
    RETURN OLD;
END;
$$
LANGUAGE plpgsql;

-- Trigger function to delete the nutrients, preview image and full image when a product description is deleted
CREATE OR REPLACE FUNCTION trigger_func_delete_product_description()
    RETURNS TRIGGER
    AS $$
BEGIN
    DELETE FROM nutrients
    WHERE id = OLD.nutrients;
    DELETE FROM product_image
    WHERE id = OLD.preview;
    DELETE FROM product_image
    WHERE id = OLD.photo;
    RETURN OLD;
END;
$$
LANGUAGE plpgsql;

--
-- DEFINITION OF TRIGGERS
--
-- Trigger to delete the product description when a product request is deleted
CREATE TRIGGER trigger_delete_requested_product
    AFTER DELETE ON requested_products
    FOR EACH ROW
    EXECUTE FUNCTION trigger_func_delete_product_or_requested_product();

-- Trigger to delete the product description when a product is deleted
CREATE TRIGGER trigger_delete_product
    AFTER DELETE ON products
    FOR EACH ROW
    EXECUTE FUNCTION trigger_func_delete_product_or_requested_product();

-- Trigger to delete the nutrients, preview image and full image when a product description is deleted
CREATE TRIGGER trigger_delete_product_description
    AFTER DELETE ON product_description
    FOR EACH ROW
    EXECUTE FUNCTION trigger_func_delete_product_description();

