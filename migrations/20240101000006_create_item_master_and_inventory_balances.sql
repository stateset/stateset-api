-- ITEM MASTER
CREATE TABLE public.item_master (
  inventory_item_id BIGSERIAL PRIMARY KEY,
  organization_id BIGINT NOT NULL,
  item_number VARCHAR NOT NULL,
  description TEXT,
  primary_uom_code VARCHAR,
  item_type VARCHAR,
  status_code VARCHAR,
  lead_time_weeks INTEGER,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- INVENTORY LOCATIONS
CREATE TABLE public.inventory_locations (
  location_id SERIAL PRIMARY KEY,
  location_name VARCHAR NOT NULL
);

-- INVENTORY BALANCES
CREATE TABLE public.inventory_balances (
  inventory_balance_id BIGSERIAL PRIMARY KEY,
  inventory_item_id BIGINT NOT NULL,
  location_id INTEGER NOT NULL,
  quantity_on_hand NUMERIC DEFAULT 0,
  quantity_allocated NUMERIC DEFAULT 0,
  quantity_available NUMERIC GENERATED ALWAYS AS (quantity_on_hand - quantity_allocated) STORED,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (inventory_item_id) REFERENCES public.item_master (inventory_item_id),
  FOREIGN KEY (location_id) REFERENCES public.inventory_locations (location_id)
);

-- BOM HEADERS
CREATE TABLE public.bom_headers (
  bom_id BIGSERIAL PRIMARY KEY,
  bom_name VARCHAR NOT NULL,
  item_id BIGINT,
  organization_id BIGINT NOT NULL,
  revision VARCHAR,
  status_code VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (item_id) REFERENCES public.item_master (inventory_item_id)
);

-- BOM LINES
CREATE TABLE public.bom_lines (
  bom_line_id BIGSERIAL PRIMARY KEY,
  bom_id BIGINT,
  component_item_id BIGINT,
  quantity_per_assembly NUMERIC,
  uom_code VARCHAR,
  operation_seq_num INTEGER,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (bom_id) REFERENCES public.bom_headers (bom_id),
  FOREIGN KEY (component_item_id) REFERENCES public.item_master (inventory_item_id)
);

-- MANUFACTURING WORK ORDERS
CREATE TABLE public.manufacturing_work_orders (
  work_order_id BIGSERIAL PRIMARY KEY,
  work_order_number VARCHAR NOT NULL,
  item_id BIGINT,
  organization_id BIGINT NOT NULL,
  scheduled_start_date DATE,
  scheduled_completion_date DATE,
  actual_start_date DATE,
  actual_completion_date DATE,
  status_code VARCHAR,
  quantity_to_build NUMERIC,
  quantity_completed NUMERIC,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (item_id) REFERENCES public.item_master (inventory_item_id)
);

-- SALES ORDER HEADERS
CREATE TABLE public.sales_order_headers (
  header_id BIGSERIAL PRIMARY KEY,
  order_number VARCHAR NOT NULL,
  order_type_id BIGINT,
  sold_to_org_id BIGINT,
  ordered_date DATE,
  status_code VARCHAR,
  location_id INTEGER,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- SALES ORDER LINES
CREATE TABLE public.sales_order_lines (
  line_id BIGSERIAL PRIMARY KEY,
  header_id BIGINT,
  line_number INTEGER,
  inventory_item_id BIGINT,
  ordered_quantity NUMERIC,
  unit_selling_price NUMERIC,
  line_status VARCHAR,
  location_id INTEGER,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (header_id) REFERENCES public.sales_order_headers (header_id),
  FOREIGN KEY (inventory_item_id) REFERENCES public.item_master (inventory_item_id)
);

-- ORDER FULFILLMENTS
CREATE TABLE public.order_fulfillments (
  fulfillment_id BIGSERIAL PRIMARY KEY,
  sales_order_header_id BIGINT,
  sales_order_line_id BIGINT,
  shipped_date DATE,
  released_status VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (sales_order_header_id) REFERENCES public.sales_order_headers (header_id),
  FOREIGN KEY (sales_order_line_id) REFERENCES public.sales_order_lines (line_id)
);

-- PURCHASE ORDER HEADERS
CREATE TABLE public.purchase_order_headers (
  po_header_id BIGSERIAL PRIMARY KEY,
  po_number VARCHAR NOT NULL,
  type_code VARCHAR,
  vendor_id BIGINT,
  agent_id BIGINT,
  approved_flag BOOLEAN,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- PURCHASE ORDER LINES
CREATE TABLE public.purchase_order_lines (
  po_line_id BIGSERIAL PRIMARY KEY,
  po_header_id BIGINT,
  line_num INTEGER,
  item_id BIGINT,
  quantity NUMERIC,
  unit_price NUMERIC,
  line_type_id BIGINT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (po_header_id) REFERENCES public.purchase_order_headers (po_header_id),
  FOREIGN KEY (item_id) REFERENCES public.item_master (inventory_item_id)
);

-- PURCHASE ORDER DISTRIBUTIONS
CREATE TABLE public.purchase_order_distributions (
  po_distribution_id BIGSERIAL PRIMARY KEY,
  po_line_id BIGINT,
  quantity_ordered NUMERIC,
  destination_type VARCHAR,
  charge_account_id BIGINT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (po_line_id) REFERENCES public.purchase_order_lines (po_line_id)
);

-- PURCHASE INVOICES
CREATE TABLE public.purchase_invoices (
  ap_invoice_id BIGSERIAL PRIMARY KEY,
  invoice_num VARCHAR NOT NULL,
  vendor_id BIGINT,
  invoice_date DATE,
  invoice_amount NUMERIC,
  status VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- PURCHASE INVOICE LINES
CREATE TABLE public.purchase_invoice_lines (
  ap_invoice_line_id BIGSERIAL PRIMARY KEY,
  ap_invoice_id BIGINT,
  line_type_code VARCHAR,
  amount NUMERIC,
  quantity NUMERIC,
  po_header_id BIGINT,
  po_line_id BIGINT,
  sku VARCHAR,
  po_number VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (ap_invoice_id) REFERENCES public.purchase_invoices (ap_invoice_id),
  FOREIGN KEY (po_header_id) REFERENCES public.purchase_order_headers (po_header_id),
  FOREIGN KEY (po_line_id) REFERENCES public.purchase_order_lines (po_line_id)
);

-- PO RECEIPT HEADERS
CREATE TABLE public.po_receipt_headers (
  shipment_header_id BIGSERIAL PRIMARY KEY,
  receipt_num VARCHAR NOT NULL,
  vendor_id BIGINT,
  shipment_num VARCHAR,
  receipt_source VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- PO RECEIPT LINES
CREATE TABLE public.po_receipt_lines (
  shipment_line_id BIGSERIAL PRIMARY KEY,
  shipment_header_id BIGINT,
  item_id BIGINT,
  po_header_id BIGINT,
  po_line_id BIGINT,
  quantity_received NUMERIC,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (shipment_header_id) REFERENCES public.po_receipt_headers (shipment_header_id),
  FOREIGN KEY (item_id) REFERENCES public.item_master (inventory_item_id),
  FOREIGN KEY (po_header_id) REFERENCES public.purchase_order_headers (po_header_id),
  FOREIGN KEY (po_line_id) REFERENCES public.purchase_order_lines (po_line_id)
);

-- SALES INVOICES
CREATE TABLE public.sales_invoices (
  invoice_id BIGSERIAL PRIMARY KEY,
  trx_number VARCHAR NOT NULL,
  bill_to_customer_id BIGINT,
  trx_date DATE,
  trx_type VARCHAR,
  status VARCHAR,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now()
);

-- SALES INVOICE LINES
CREATE TABLE public.sales_invoice_lines (
  invoice_line_id BIGSERIAL PRIMARY KEY,
  invoice_id BIGINT,
  line_number INTEGER,
  description TEXT,
  quantity_invoiced NUMERIC,
  unit_selling_price NUMERIC,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  FOREIGN KEY (invoice_id) REFERENCES public.sales_invoices (invoice_id)
);

-- CREATE INDEXES FOR BETTER PERFORMANCE
-- Item Master indexes
CREATE INDEX idx_item_master_organization ON public.item_master(organization_id);
CREATE INDEX idx_item_master_item_number ON public.item_master(item_number);
CREATE INDEX idx_item_master_status ON public.item_master(status_code);

-- Inventory indexes
CREATE INDEX idx_inventory_balances_item ON public.inventory_balances(inventory_item_id);
CREATE INDEX idx_inventory_balances_location ON public.inventory_balances(location_id);
CREATE INDEX idx_inventory_balances_item_location ON public.inventory_balances(inventory_item_id, location_id);
CREATE UNIQUE INDEX idx_inventory_balances_unique_item_location ON public.inventory_balances(inventory_item_id, location_id);

-- BOM indexes
CREATE INDEX idx_bom_headers_item ON public.bom_headers(item_id);
CREATE INDEX idx_bom_lines_bom ON public.bom_lines(bom_id);
CREATE INDEX idx_bom_lines_component ON public.bom_lines(component_item_id);

-- Manufacturing indexes
CREATE INDEX idx_manufacturing_work_orders_item ON public.manufacturing_work_orders(item_id);
CREATE INDEX idx_manufacturing_work_orders_status ON public.manufacturing_work_orders(status_code);

-- Sales Order indexes
CREATE INDEX idx_sales_order_headers_order_number ON public.sales_order_headers(order_number);
CREATE INDEX idx_sales_order_headers_status ON public.sales_order_headers(status_code);
CREATE INDEX idx_sales_order_lines_header ON public.sales_order_lines(header_id);
CREATE INDEX idx_sales_order_lines_item ON public.sales_order_lines(inventory_item_id);

-- Purchase Order indexes
CREATE INDEX idx_purchase_order_headers_po_number ON public.purchase_order_headers(po_number);
CREATE INDEX idx_purchase_order_headers_vendor ON public.purchase_order_headers(vendor_id);
CREATE INDEX idx_purchase_order_lines_header ON public.purchase_order_lines(po_header_id);
CREATE INDEX idx_purchase_order_lines_item ON public.purchase_order_lines(item_id);

-- Invoice indexes
CREATE INDEX idx_purchase_invoices_vendor ON public.purchase_invoices(vendor_id);
CREATE INDEX idx_purchase_invoice_lines_invoice ON public.purchase_invoice_lines(ap_invoice_id);
CREATE INDEX idx_sales_invoices_customer ON public.sales_invoices(bill_to_customer_id);
CREATE INDEX idx_sales_invoice_lines_invoice ON public.sales_invoice_lines(invoice_id);

-- Receipt indexes
CREATE INDEX idx_po_receipt_headers_vendor ON public.po_receipt_headers(vendor_id);
CREATE INDEX idx_po_receipt_lines_header ON public.po_receipt_lines(shipment_header_id); 