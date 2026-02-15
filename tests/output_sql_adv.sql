CREATE OR REPLACE FUNCTION is_adult(age BIGINT) RETURNS BOOLEAN AS $$
BEGIN
    RETURN age >= 18;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION calculate_discount(price BIGINT) RETURNS BIGINT AS $$
BEGIN
    discount := 10;
    RETURN price - discount;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION main() RETURNS VOID AS $$
BEGIN
    price := 100;
    discount := calculate_discount(price);
    IF is_adult(20) THEN
    PERFORM io.println(NULL);
    END IF;
    RETURN discount;
END;
$$ LANGUAGE plpgsql;

