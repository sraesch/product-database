-- insert the nutrients for a new food item
insert into nutrients (kcal, protein_grams, fat_grams, carbohydrates_grams, sugar_grams, salt_grams, calcium_mg) values (40, 0.2, 1.5, 5.6, 0, 0.09, 0.12) RETURNING id;

-- create a new product description
insert into product_description (product_id, name, producer, quantity_type, portion, volume_weight_ratio, nutrients) values ('5411188124689', 'Haferdrink ungesüßt, 1 Liter', 'Alpro', 'volume', 100, 1.0, 1) RETURNING id;

-- create a new product request
insert into requested_products (product_description_id, date) values (1, '2021-01-01 12:00:00') RETURNING id;
