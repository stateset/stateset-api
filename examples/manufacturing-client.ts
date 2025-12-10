/**
 * StateSet Manufacturing API - TypeScript Client
 *
 * Comprehensive TypeScript client for manufacturing operations including:
 * - Bill of Materials (BOM) management
 * - Work order lifecycle
 * - Batch production and lot tracking
 * - Component traceability
 * - Quality control integration
 * - Production analytics
 *
 * @requires axios uuid
 * @example
 * ```bash
 * npm install axios uuid
 * npx ts-node manufacturing-client.ts
 * ```
 */

import axios, { AxiosInstance, AxiosResponse } from 'axios';
import { v4 as uuidv4 } from 'uuid';

// ========== Type Definitions ==========

interface LoginResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

interface BOM {
  bom_id: string;
  bom_name: string;
  item_id: string;
  organization_id: number;
  revision?: string;
  status: string;
}

interface BOMComponent {
  bom_line_id: string;
  bom_id: string;
  component_item_id: string;
  quantity_per_assembly: number;
  uom_code: string;
  operation_seq_num?: number;
}

interface WorkOrder {
  work_order_id: string;
  work_order_number: string;
  item_id: string;
  organization_id: number;
  quantity_to_build: number;
  quantity_completed?: number;
  status_code: string;
  scheduled_start_date: string;
  scheduled_completion_date: string;
  actual_start_date?: string;
  actual_completion_date?: string;
  location_id: number;
  priority: string;
}

interface Batch {
  id: string;
  batch_number: string;
  product_id: string;
  batch_size: number;
  production_date: string;
  expiry_date: string;
  status: string;
  location_id: number;
}

interface BatchMaterial {
  material_id: string;
  material_name: string;
  lot_number: string;
  quantity_used: number;
  unit: string;
  expiry_date?: string;
  supplier?: string;
}

interface QualityCheck {
  test_name: string;
  test_type: 'in_process' | 'final_release';
  results: Record<string, any>;
  status: 'PASS' | 'FAIL';
  tested_by: string;
  test_time: string;
}

interface ComponentSerial {
  id: string;
  serial_number: string;
  component_type: string;
  component_sku: string;
  supplier_id: string;
  supplier_lot_number: string;
  manufacture_date: string;
  receive_date: string;
  location: string;
}

interface RobotSerial {
  id: string;
  serial_number: string;
  robot_model: string;
  robot_type: string;
  product_id: string;
  work_order_id: string;
  manufacturing_date: string;
  status: string;
}

interface ProductionMetrics {
  work_orders_created: number;
  work_orders_completed: number;
  total_units_produced: number;
  average_yield_percent: number;
  average_cycle_time_days: number;
}

// ========== Client Class ==========

export class ManufacturingClient {
  private client: AxiosInstance;
  private token: string | null = null;

  constructor(baseURL: string = 'http://localhost:8080/api/v1') {
    this.client = axios.create({
      baseURL,
      headers: {
        'Content-Type': 'application/json',
      },
    });

    // Add request interceptor to include token
    this.client.interceptors.request.use((config) => {
      if (this.token) {
        config.headers.Authorization = `Bearer ${this.token}`;
      }
      return config;
    });
  }

  // ========== Authentication ==========

  async login(email: string, password: string): Promise<LoginResponse> {
    const response = await this.client.post<LoginResponse>('/auth/login', {
      email,
      password,
    });
    this.token = response.data.access_token;
    console.log(`✓ Logged in as ${email}`);
    return response.data;
  }

  // ========== BOM Management ==========

  async createBOM(params: {
    bom_name: string;
    item_id: string;
    organization_id: number;
    revision?: string;
  }): Promise<BOM> {
    const response = await this.client.post<BOM>('/manufacturing/boms', params);
    return response.data;
  }

  async addBOMComponent(params: {
    bom_id: string;
    component_item_id: string;
    quantity_per_assembly: number;
    uom_code: string;
    operation_seq_num?: number;
  }): Promise<BOMComponent> {
    const { bom_id, ...data } = params;
    const response = await this.client.post<BOMComponent>(
      `/manufacturing/boms/${bom_id}/components`,
      data
    );
    return response.data;
  }

  async getBOM(bomId: string): Promise<BOM> {
    const response = await this.client.get<BOM>(`/manufacturing/boms/${bomId}`);
    return response.data;
  }

  async explodeBOM(params: {
    item_id: string;
    quantity: number;
    level?: number;
  }): Promise<any[]> {
    const response = await this.client.post('/manufacturing/boms/explode', params);
    return response.data;
  }

  // ========== Work Order Management ==========

  async createWorkOrder(params: {
    work_order_number: string;
    item_id: string;
    quantity_to_build: number;
    scheduled_start_date: string;
    scheduled_completion_date: string;
    location_id: number;
    organization_id?: number;
    priority?: string;
  }): Promise<WorkOrder> {
    const response = await this.client.post<WorkOrder>('/work-orders', {
      organization_id: 1,
      priority: 'MEDIUM',
      ...params,
    });
    return response.data;
  }

  async startWorkOrder(
    workOrderId: string,
    locationId: number,
    operatorId?: string
  ): Promise<WorkOrder> {
    const response = await this.client.post<WorkOrder>(
      `/work-orders/${workOrderId}/start`,
      {
        location_id: locationId,
        operator_id: operatorId,
      }
    );
    return response.data;
  }

  async completeWorkOrder(
    workOrderId: string,
    completedQuantity: number,
    locationId: number
  ): Promise<WorkOrder> {
    const response = await this.client.post<WorkOrder>(
      `/work-orders/${workOrderId}/complete`,
      {
        completed_quantity: completedQuantity,
        location_id: locationId,
      }
    );
    return response.data;
  }

  async holdWorkOrder(workOrderId: string, reason?: string): Promise<WorkOrder> {
    const response = await this.client.put<WorkOrder>(
      `/work-orders/${workOrderId}/hold`,
      { reason }
    );
    return response.data;
  }

  async resumeWorkOrder(workOrderId: string): Promise<WorkOrder> {
    const response = await this.client.put<WorkOrder>(
      `/work-orders/${workOrderId}/resume`
    );
    return response.data;
  }

  async cancelWorkOrder(
    workOrderId: string,
    locationId: number
  ): Promise<void> {
    await this.client.delete(`/work-orders/${workOrderId}`, {
      data: { location_id: locationId },
    });
  }

  async getWorkOrder(workOrderId: string): Promise<WorkOrder> {
    const response = await this.client.get<WorkOrder>(
      `/work-orders/${workOrderId}`
    );
    return response.data;
  }

  async listWorkOrders(params?: {
    status?: string;
    page?: number;
    limit?: number;
  }): Promise<{ data: WorkOrder[]; total: number }> {
    const response = await this.client.get('/work-orders', {
      params: {
        page: 1,
        limit: 20,
        ...params,
      },
    });
    return response.data;
  }

  // ========== Batch Production ==========

  async createBatch(params: {
    batch_number: string;
    product_id: string;
    batch_size: number;
    production_date: string;
    expiry_date: string;
    location_id: number;
  }): Promise<Batch> {
    const response = await this.client.post<Batch>('/manufacturing/batches', {
      ...params,
      status: 'PLANNED',
    });
    return response.data;
  }

  async addBatchMaterials(
    batchId: string,
    materials: BatchMaterial[]
  ): Promise<void> {
    await this.client.post(`/manufacturing/batches/${batchId}/materials`, {
      materials,
    });
  }

  async startBatch(params: {
    batch_id: string;
    operator_id: string;
    equipment_id?: string;
  }): Promise<Batch> {
    const { batch_id, ...data } = params;
    const response = await this.client.put<Batch>(
      `/manufacturing/batches/${batch_id}/start`,
      {
        ...data,
        started_by: params.operator_id,
        start_time: new Date().toISOString(),
      }
    );
    return response.data;
  }

  async addQualityCheck(
    batchId: string,
    check: QualityCheck
  ): Promise<void> {
    await this.client.post(`/manufacturing/batches/${batchId}/quality-checks`, {
      ...check,
      test_time: new Date().toISOString(),
    });
  }

  async completeBatch(params: {
    batch_id: string;
    actual_quantity: number;
    completed_by: string;
    yield_percentage: number;
  }): Promise<Batch> {
    const { batch_id, ...data } = params;
    const response = await this.client.put<Batch>(
      `/manufacturing/batches/${batch_id}/complete`,
      {
        ...data,
        actual_quantity_produced: data.actual_quantity,
        completion_time: new Date().toISOString(),
      }
    );
    return response.data;
  }

  async releaseBatch(params: {
    batch_id: string;
    released_by: string;
    review_notes: string;
  }): Promise<Batch> {
    const { batch_id, ...data } = params;
    const response = await this.client.put<Batch>(
      `/manufacturing/batches/${batch_id}/release`,
      {
        ...data,
        release_date: new Date().toISOString(),
        release_status: 'APPROVED',
      }
    );
    return response.data;
  }

  async getBatchGenealogy(batchId: string): Promise<any> {
    const response = await this.client.get(
      `/manufacturing/batches/${batchId}/genealogy`
    );
    return response.data;
  }

  // ========== Component & Robot Tracking ==========

  async createComponentSerial(params: {
    serial_number: string;
    component_type: string;
    component_sku: string;
    supplier_id: string;
    supplier_lot_number: string;
    manufacture_date: string;
    receive_date: string;
    location: string;
  }): Promise<ComponentSerial> {
    const response = await this.client.post<ComponentSerial>(
      '/manufacturing/components/serials',
      params
    );
    return response.data;
  }

  async createRobotSerial(params: {
    serial_number: string;
    robot_model: string;
    robot_type: string;
    product_id: string;
    work_order_id: string;
    manufacturing_date: string;
  }): Promise<RobotSerial> {
    const response = await this.client.post<RobotSerial>(
      '/manufacturing/robots/serials',
      params
    );
    return response.data;
  }

  async installComponent(params: {
    robot_serial_id: string;
    component_serial_id: string;
    position: string;
    installed_by: string;
  }): Promise<void> {
    await this.client.post('/manufacturing/components/install', params);
  }

  async addTestResult(params: {
    test_protocol_id: string;
    robot_serial_id: string;
    tested_by: string;
    status: string;
    measurements: Record<string, any>;
    notes?: string;
  }): Promise<void> {
    await this.client.post('/manufacturing/test-results', params);
  }

  async getRobotGenealogy(robotSerialId: string): Promise<any> {
    const response = await this.client.get(
      `/manufacturing/robots/serials/${robotSerialId}/genealogy`
    );
    return response.data;
  }

  // ========== Analytics ==========

  async getProductionMetrics(params: {
    start_date: string;
    end_date: string;
  }): Promise<ProductionMetrics> {
    const response = await this.client.get<ProductionMetrics>(
      '/analytics/manufacturing/production',
      { params }
    );
    return response.data;
  }

  async getWorkOrderAnalytics(): Promise<any> {
    const response = await this.client.get(
      '/analytics/manufacturing/work-orders'
    );
    return response.data;
  }

  async getYieldAnalysis(productId?: string): Promise<any> {
    const response = await this.client.get('/analytics/manufacturing/yield', {
      params: productId ? { product_id: productId } : {},
    });
    return response.data;
  }
}

// ========== Example Functions ==========

async function exampleWorkOrderLifecycle() {
  console.log('\n' + '='.repeat(60));
  console.log('Example 1: Work Order Lifecycle');
  console.log('='.repeat(60) + '\n');

  const client = new ManufacturingClient();
  await client.login('admin@stateset.com', 'your-password');

  // Create BOM
  console.log('Creating BOM...');
  const bom = await client.createBOM({
    bom_name: 'BOM-WIDGET-001',
    item_id: 'item-widget-001',
    organization_id: 1,
    revision: '1.0',
  });
  console.log(`✓ BOM created: ${bom.bom_id}`);

  // Add components
  console.log('\nAdding components to BOM...');
  await client.addBOMComponent({
    bom_id: bom.bom_id,
    component_item_id: 'item-screw-001',
    quantity_per_assembly: 4.0,
    uom_code: 'EA',
    operation_seq_num: 10,
  });
  console.log('✓ Added screws (4 per unit)');

  // Create work order
  console.log('\nCreating work order...');
  const today = new Date();
  const fiveDaysLater = new Date(today);
  fiveDaysLater.setDate(today.getDate() + 5);

  let wo = await client.createWorkOrder({
    work_order_number: `WO-${today.toISOString().split('T')[0].replace(/-/g, '')}-001`,
    item_id: 'item-widget-001',
    quantity_to_build: 100.0,
    scheduled_start_date: today.toISOString().split('T')[0],
    scheduled_completion_date: fiveDaysLater.toISOString().split('T')[0],
    location_id: 100,
    priority: 'HIGH',
  });
  console.log(`✓ Work order created: ${wo.work_order_number}`);
  console.log(`  Status: ${wo.status_code}`);

  // Start work order
  console.log('\nStarting work order...');
  wo = await client.startWorkOrder(wo.work_order_id, 100, 'operator-001');
  console.log(`✓ Work order started`);
  console.log(`  Status: ${wo.status_code}`);

  // Complete work order
  console.log('\nCompleting work order...');
  wo = await client.completeWorkOrder(wo.work_order_id, 100.0, 100);
  console.log(`✓ Work order completed`);
  console.log(`  Status: ${wo.status_code}`);
  console.log(`  Completed: ${wo.quantity_completed}/${wo.quantity_to_build}`);

  console.log('\n✓ Work order lifecycle complete!');
}

async function exampleBatchProduction() {
  console.log('\n' + '='.repeat(60));
  console.log('Example 2: Batch Production with Quality Control');
  console.log('='.repeat(60) + '\n');

  const client = new ManufacturingClient();
  await client.login('admin@stateset.com', 'your-password');

  // Create batch
  console.log('Creating batch...');
  const today = new Date();
  const expiry = new Date(today);
  expiry.setFullYear(expiry.getFullYear() + 2);

  const batch = await client.createBatch({
    batch_number: `BATCH-${today.toISOString().split('T')[0].replace(/-/g, '')}-001`,
    product_id: 'prod-tablet-001',
    batch_size: 100000,
    production_date: today.toISOString().split('T')[0],
    expiry_date: expiry.toISOString().split('T')[0],
    location_id: 100,
  });
  console.log(`✓ Batch created: ${batch.batch_number}`);

  // Add materials
  console.log('\nRecording batch materials...');
  const materials: BatchMaterial[] = [
    {
      material_id: 'rm-001',
      material_name: 'Active Ingredient',
      lot_number: 'LOT-2024-001',
      quantity_used: 25.0,
      unit: 'kg',
    },
    {
      material_id: 'rm-002',
      material_name: 'Excipient',
      lot_number: 'LOT-2024-002',
      quantity_used: 45.0,
      unit: 'kg',
    },
  ];
  await client.addBatchMaterials(batch.id, materials);
  console.log('✓ Materials recorded');

  // Start batch
  console.log('\nStarting batch production...');
  await client.startBatch({
    batch_id: batch.id,
    operator_id: 'operator-001',
    equipment_id: 'PRESS-01',
  });
  console.log('✓ Batch started');

  // Add quality checks
  console.log('\nPerforming quality checks...');
  await client.addQualityCheck(batch.id, {
    test_name: 'Weight Variation',
    test_type: 'in_process',
    results: {
      average_weight_mg: 626.3,
      within_spec: true,
    },
    status: 'PASS',
    tested_by: 'qc-analyst-001',
    test_time: new Date().toISOString(),
  });
  console.log('✓ Weight variation test: PASS');

  // Complete batch
  console.log('\nCompleting batch...');
  await client.completeBatch({
    batch_id: batch.id,
    actual_quantity: 98500,
    completed_by: 'operator-001',
    yield_percentage: 98.5,
  });
  console.log('✓ Batch completed (98.5% yield)');

  // Release batch
  console.log('\nReleasing batch...');
  await client.releaseBatch({
    batch_id: batch.id,
    released_by: 'qa-manager-001',
    review_notes: 'All tests passed. Approved for distribution.',
  });
  console.log('✓ Batch released');

  console.log('\n✓ Batch production complete!');
}

async function exampleProductionAnalytics() {
  console.log('\n' + '='.repeat(60));
  console.log('Example 3: Production Analytics');
  console.log('='.repeat(60) + '\n');

  const client = new ManufacturingClient();
  await client.login('admin@stateset.com', 'your-password');

  // Get production metrics
  console.log('Fetching production metrics...');
  const today = new Date();
  const thirtyDaysAgo = new Date(today);
  thirtyDaysAgo.setDate(today.getDate() - 30);

  const metrics = await client.getProductionMetrics({
    start_date: thirtyDaysAgo.toISOString().split('T')[0],
    end_date: today.toISOString().split('T')[0],
  });
  console.log('✓ Production metrics (last 30 days):');
  console.log(`  Work orders created: ${metrics.work_orders_created}`);
  console.log(`  Work orders completed: ${metrics.work_orders_completed}`);
  console.log(`  Units produced: ${metrics.total_units_produced}`);
  console.log(`  Average yield: ${metrics.average_yield_percent.toFixed(1)}%`);

  // Get work order analytics
  console.log('\nFetching work order analytics...');
  const woAnalytics = await client.getWorkOrderAnalytics();
  console.log('✓ Work order analytics:');
  console.log(`  Active orders: ${woAnalytics.active_count || 0}`);
  console.log(`  On hold: ${woAnalytics.on_hold_count || 0}`);
  console.log(`  Average cycle time: ${(woAnalytics.avg_cycle_time_days || 0).toFixed(1)} days`);

  console.log('\n✓ Analytics retrieved successfully!');
}

// ========== Main Execution ==========

async function main() {
  console.log('\n' + '='.repeat(60));
  console.log('StateSet Manufacturing API - TypeScript Examples');
  console.log('='.repeat(60));

  try {
    await exampleWorkOrderLifecycle();
    await exampleBatchProduction();
    await exampleProductionAnalytics();

    console.log('\n' + '='.repeat(60));
    console.log('All examples completed successfully!');
    console.log('='.repeat(60) + '\n');
  } catch (error: any) {
    console.error('\n❌ Error:', error.message);
    if (error.response) {
      console.error('Response:', error.response.data);
    }
  }
}

// Run examples if this file is executed directly
if (require.main === module) {
  main();
}

// Export for use as module
export default ManufacturingClient;
