use std::collections::{HashMap, HashSet};
use rust_decimal::Decimal;
use serde::Serialize;

use super::tree::{delete_limit, insert_recursive};

#[derive(Debug, Clone)]
pub enum BidOrAsk {
  Bid,
  Ask,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutedOrders {
  price: Decimal,
  volume: u64,
  aggresive_order_id: u64,
  passive_order_id: u64,
}

#[derive(Debug)]
pub struct Order {
  id_number: u64,
  pub bid_or_ask: BidOrAsk,
  shares: u64,
  limit: Decimal,

  next_order: Option<u64>,
  prev_order: Option<u64>,
  parent_limit: Option<Decimal>
}

impl Order {
  fn new(_id_number: u64, _bid_or_ask: BidOrAsk, _shares: u64, _limit: Decimal) -> Self {
      Order { id_number: (_id_number), 
        bid_or_ask: (_bid_or_ask),
          shares: (_shares), limit: (_limit),
        next_order: None, prev_order: None, parent_limit: None }
  }

  fn modify_order(&mut self, new_shares: u64, new_limit_price: Decimal) {
    self.shares = new_shares;
    self.limit = new_limit_price;
    (self.next_order , self.prev_order , self.parent_limit) = (None, None, None);
  }

}

#[derive(Debug)]
pub struct Limit {
  pub limit_price: Decimal,
  size: u64,
  pub total_volume: u64,
  pub bid_or_ask: BidOrAsk, 

  pub parent: Option<Decimal>,
  pub left_child: Option<Decimal>,
  pub right_child: Option<Decimal>,

  head_order: Option<u64>,
  tail_order: Option<u64>,
  pub height: i32
}

impl Limit {
  fn new(_limit_price: Decimal, _size: u64, _total_volume: u64, _bid_or_ask: BidOrAsk) -> Self {
    Limit { parent: None, left_child: None, right_child: None, head_order: None, tail_order: None, limit_price: _limit_price, size: _size, total_volume: _total_volume, bid_or_ask: _bid_or_ask, height: 1 }

  }

  fn partially_fill_total_volume(&mut self, ordered_shares: u64) {
    self.total_volume -= ordered_shares;
  }

  fn append(&mut self, order_id: u64, order_map: &mut HashMap<u64, Order>) {
    // Borrow order at order_id
    let (head_order, tail_order) = (&mut self.head_order, &mut self.tail_order);

    match head_order {
      None => {
        // If the list is empty, initialize head and tail to the new order
        *head_order = Some(order_id);
        *tail_order = Some(order_id);
      }
      Some(_) => {
        // First, modify the current tail entry if it exists
        if let Some(current_tail) = tail_order {
            // let previous_tail = &mut order_map[current_tail];

            if let Some(previous_tail) = order_map.get_mut(current_tail) {
              previous_tail.next_order = Some(order_id);
            }
        }
        // Now, modify the new order and update it as the new tail
        if let Some(new_order) = order_map.get_mut(&order_id) {
          new_order.prev_order = *tail_order;
          new_order.next_order = None;
        }
        // Update the tail to the new order
        *tail_order = Some(order_id);
      }
    }
    self.size += 1;
    self.total_volume += order_map[&order_id].shares;
    // order_map[order_id].parent_limit = Some(self.limit_price);
    if let Some(new_order) = order_map.get_mut(&order_id) {
      new_order.parent_limit = Some(self.limit_price);
    }
  }
}

pub struct Arena {
  pub buy_limits: HashMap<Decimal, Limit>,
  pub sell_limits: HashMap<Decimal, Limit>,
  pub orders: HashMap<u64, Order>,
  limit_orders: HashSet<u64>,
  executed_orders: Vec<ExecutedOrders>,

  pub highest_buy: Option<Decimal>,
  pub lowest_sell: Option<Decimal>,
  buy_tree: Option<Decimal>,
  sell_tree: Option<Decimal>,
  pub executed_orders_count: usize,
  pub avl_rebalances: u64,
  show_best_price_levels: bool
}

impl Arena {
  pub fn new(best_price_lvls: bool) -> Self {
    Arena {buy_limits: HashMap::new(), sell_limits: HashMap::new(), orders: HashMap::new(), limit_orders: HashSet::new(), executed_orders: Vec::new(), highest_buy: None, lowest_sell: None, buy_tree: None, sell_tree: None, executed_orders_count: 0, avl_rebalances: 0, show_best_price_levels: best_price_lvls} 
  }

  pub fn get_executed_orders(&mut self, offset: &mut usize) -> Option<Vec<ExecutedOrders>> {
    let mut fresh_trades = None;
    if self.executed_orders_count > 0 && *offset != self.executed_orders_count {
      let trades = &self.executed_orders[*offset..];
      *offset = self.executed_orders.len(); // update the offset
      fresh_trades = Some(trades.to_vec());

    }
    fresh_trades
  }

  pub fn get_top_n_bids(&self, n: usize) -> Vec<(Decimal, u64)> {
    let mut best_bids: Vec<(Decimal, u64)> = Vec::new();
    
    if self.show_best_price_levels {
      if let Some(max_bid) = self.highest_buy {
        self.collect_n_bids(max_bid, n, &mut best_bids);
        // println!("highest buy limit: {:?}", self.buy_limits.get(&max_bid).unwrap());
      }
    } else {
      if let Some(buy_root) = self.buy_tree {
        self.collect_n_bids(buy_root, n, &mut best_bids);
      }
    }
    // println!("best bids length: {:?}", best_bids.len());
    best_bids
  }

  pub fn get_top_n_asks(&self, n: usize) -> Vec<(Decimal, u64)> {
    let mut best_asks: Vec<(Decimal, u64)> = Vec::new();

    if self.show_best_price_levels {
      if let Some(min_ask) = self.lowest_sell {
        self.collect_n_asks(min_ask, n, &mut best_asks);
        // println!("lowest sell limit: {:?}", self.sell_limits.get(&min_ask).unwrap());
      }
    } else {
      if let Some(sell_root) = self.sell_tree {
        self.collect_n_asks(sell_root, n, &mut best_asks);
      }   
    }
    // println!("best asks length: {:?}", best_asks.len());
    best_asks
  }

  fn collect_n_bids(&self, price: Decimal, n: usize, best_bids: &mut Vec<(Decimal, u64)>) {
    
    if best_bids.len() >= n {
      return;
    }

    if let Some(limit) = self.buy_limits.get(&price) {
      // traverse right subtree first (higher prices)
      if let Some(right_child) = limit.right_child {
        self.collect_n_bids(right_child, n, best_bids);
      }

      if best_bids.len() < n {
        best_bids.push((price, limit.total_volume));
      }

      // Traverse left subtree (lower prices)
      if let Some(left_child) = limit.left_child {
        self.collect_n_bids(left_child, n, best_bids);
      }
    }
  }

  fn collect_n_asks(&self, price: Decimal, n: usize, best_asks: &mut Vec<(Decimal, u64)>) {
    
    if best_asks.len() >= n {
      return;
    }

    if let Some(limit) = self.sell_limits.get(&price) {
      // Traverse left subtree first (lower prices)
      if let Some(left_child) = limit.left_child {
        self.collect_n_asks(left_child, n, best_asks);
      }

      if best_asks.len() < n {
        best_asks.push((price, limit.total_volume));
      }

      // traverse right subtree (higher prices)
      if let Some(right_child) = limit.right_child {
        self.collect_n_asks(right_child, n, best_asks);
      }
    }
  }

  pub fn get_random_order_id(&self) -> Option<&u64> {
    //TODO: remove assert when tested!
    // also see we can remove hardcoded `10_000`
    assert_eq!(self.orders.len(), self.limit_orders.len(), "length of order map and limit order should match since we only have limit orders");

    if self.limit_orders.len() > 10_000 {
      // each iter creates a random ordering
      let mut iter = self.limit_orders.iter();
      let id = iter.next();
      return id;
    }
    None
  }

  fn limit_order_as_market_order(&mut self, order_id: &u64, bid_or_ask: &BidOrAsk, shares: &mut u64, limit_price: &Decimal) -> u64 {
    match bid_or_ask {
      BidOrAsk::Bid => {
        while  self.lowest_sell.is_some() && *shares != 0 && self.lowest_sell.unwrap() <= *limit_price {
          let lowest_sell_price = self.lowest_sell.unwrap();

          if *shares <= self.sell_limits[&lowest_sell_price].total_volume {
            //TODO: marketOrderHelper cpp starts here. 
            self.market_order_helper(order_id, bid_or_ask, shares);
            return 0;

          } else {
            // partial order fullfillment
            let mut total_volume = self.sell_limits[&lowest_sell_price].total_volume;
            *shares -= total_volume;
            self.market_order_helper(order_id, bid_or_ask, &mut total_volume);
          }
        }
        *shares
      },
      BidOrAsk::Ask => {
        
        while  self.highest_buy.is_some() && *shares != 0 && self.highest_buy.unwrap() >= *limit_price {
          // TODO: See if we can smartly unwrap highest_buy once and then use
          let highest_buy_price = self.highest_buy.unwrap();
          if *shares <= self.buy_limits[&highest_buy_price].total_volume {
            //marketOrderHelper cpp starts here. 
            self.market_order_helper(order_id, bid_or_ask, shares);
            return 0;
          } else {
            // partial order fullfillment case
            let mut total_volume = self.buy_limits[&highest_buy_price].total_volume;
            *shares -= total_volume;
            self.market_order_helper(order_id, bid_or_ask, &mut total_volume);
          }
        }
        *shares
      }
    }
  }

  fn market_order_helper(&mut self, order_id: &u64, bid_or_ask: &BidOrAsk, shares: &mut u64) {

    //NOTE: for Bids we take Ask side, i.e., sell limits, sell tree(root), lowestsell 
    //      for Asks we take Bid side, i.e., buy limits, buy tree(root),  highestbuy
    let (limit_map, tree, book_edge) = match  bid_or_ask {
      BidOrAsk::Bid => {(&mut self.sell_limits, &mut self.sell_tree, &mut self.lowest_sell)},
      BidOrAsk::Ask => {(&mut self.buy_limits, &mut self.buy_tree, &mut self.highest_buy)}
    };
    
    let mut head_order_id = limit_map[&book_edge.unwrap()].head_order.unwrap();

    while book_edge.is_some() && self.orders[&head_order_id].shares <= *shares { 
      
      let book_edge_price = book_edge.expect("book edge should exist as already checked in while clause above!!");
      let mut head_order = self.orders.remove(&head_order_id).unwrap();
      self.limit_orders.remove(&head_order_id);
      
      *shares -= head_order.shares;
      
      // start altering/executing head order
      if let Some(parent_limit) = limit_map.get_mut(&head_order.parent_limit.unwrap()) {

        parent_limit.head_order = head_order.next_order;

        if let Some(next_order) =  head_order.next_order{
          let no = self.orders.get_mut(&next_order).unwrap();
          no.prev_order = None;
        } else {
          parent_limit.tail_order = None;
        }
        head_order.next_order = None;
        head_order.prev_order = None;
        parent_limit.total_volume -= head_order.shares;
        parent_limit.size -= 1;
        
        // record the executed transactions
        self.executed_orders.push(ExecutedOrders { price: (head_order.limit), volume: (head_order.shares), aggresive_order_id: (*order_id), passive_order_id: (head_order.id_number) });
      }
      // remove book_edge if its size = 0
      if limit_map.get(&book_edge_price).expect("book edge limit should exist if the corresponding price exists while calling market order helper()!!").size == 0 {
        delete_limit(&mut self.avl_rebalances, &book_edge_price, book_edge, tree, limit_map);
      }

      self.executed_orders_count += 1;
      // update head_order_id if a book_edge exists
      if let Some(some_book_edge_price) = book_edge {
        head_order_id = limit_map[some_book_edge_price].head_order.unwrap();
      }
    }

    if book_edge.is_some() && *shares != 0 {
      let head_order = self.orders.get_mut(&head_order_id).expect("head order not found!");
      head_order.shares -= *shares;

      if let Some(parent_limit) = limit_map.get_mut(&head_order.parent_limit.unwrap()) {
        parent_limit.partially_fill_total_volume(*shares);
         // record the executed transactions
         self.executed_orders.push(ExecutedOrders { price: (head_order.limit), volume: (*shares), aggresive_order_id: (*order_id), passive_order_id: (head_order.id_number) });
        self.executed_orders_count += 1;
      } 
    }
  }

  /*TODO: Check if can be removed
  fn handle_empty_book_edge(book_edge: &mut Option<Decimal>, limit_map: &mut HashMap<Decimal, Limit>, tree: &mut Option<Decimal>) -> Limit {
    
    let book_edge_price = book_edge.unwrap();
    //first update bookedge if limit is root
    if book_edge_price == tree.unwrap() {
      match limit_map[&book_edge_price].bid_or_ask {
        BidOrAsk::Bid if !limit_map[&book_edge_price].left_child.is_none() => {
          *book_edge = limit_map[&book_edge_price].left_child
        },
        BidOrAsk::Ask if !limit_map[&book_edge_price].right_child.is_none() => {
          *book_edge = limit_map[&book_edge_price].right_child
        },
        _ => {
          *book_edge = None
        }
      }
    } else {
      match limit_map[&book_edge_price].bid_or_ask {
        BidOrAsk::Bid if !limit_map[&book_edge_price].left_child.is_none() => {
          *book_edge = limit_map[&book_edge_price].left_child
        },
        BidOrAsk::Ask if !limit_map[&book_edge_price].right_child.is_none() => {
          *book_edge = limit_map[&book_edge_price].right_child
        },
        _ => {
          *book_edge = limit_map[&book_edge_price].parent
        }
      }   
    }
    //second erase from limit map
    let book_edge_limit = limit_map.remove(&book_edge_price).unwrap();
    //third change root limit in AVL if root is deleted
    if *tree == Some(book_edge_price) {
      if book_edge_limit.right_child.is_none() {
        *tree = book_edge_limit.left_child;
      } else {
        *tree = book_edge_limit.right_child;
        let mut root_price = tree.unwrap();
        while !limit_map[&root_price].left_child.is_none() {
          *tree = limit_map[&root_price].left_child;
          root_price = tree.unwrap();
        }
      }
      // set the parent of root to None
      if let Some(tree_price) = tree {
        if let Some(limit) = limit_map.get_mut(&tree_price) {
          limit.parent = None
        }
      }
    } 

    book_edge_limit
  }
  */

  pub fn add_limit_order(&mut self, order_id: u64, bid_or_ask: BidOrAsk, mut shares: u64, limit_price: Decimal) {
    
    self.avl_rebalances = 0;
    self.executed_orders_count = 0;

    // check if order can be immediately executed (market order)
    let rem_shares = self.limit_order_as_market_order(&order_id, &bid_or_ask, &mut shares, &limit_price);

    if rem_shares != 0 {
      let new_order = Order::new(order_id, bid_or_ask.clone(), rem_shares, limit_price);
      // push new order
      self.orders.insert(order_id, new_order);
      // check for new limit and insert
      match bid_or_ask {
        BidOrAsk::Bid => {
          if !self.buy_limits.contains_key(&limit_price) {
            self.add_limit(limit_price, bid_or_ask);
          }
          // append new order to new limit
          let nl = self.buy_limits.get_mut(&limit_price).unwrap();
          // Limit::append(new_order_id, nl);
          nl.append(order_id, &mut self.orders);
          self.limit_orders.insert(order_id);
        },
        BidOrAsk::Ask => {
          if !self.sell_limits.contains_key(&limit_price) {
            self.add_limit(limit_price, bid_or_ask);
          }

          // append new order to new limit
          let nl = self.sell_limits.get_mut(&limit_price).unwrap();
          // Limit::append(new_order_id, nl);
          nl.append(order_id, &mut self.orders);
          self.limit_orders.insert(order_id);
        }
      }
    } else {
      //TODO: implement Stop Orders
    }
  }

  pub fn modify_limit_order(&mut self, order_id: u64, mut new_shares: u64, new_limit_price: Decimal) {

    self.avl_rebalances = 0;
    self.executed_orders_count = 0;
  
    let (mut are_rem_shares, mut rem_shares)= (true, None);

    // extract the order and cancel it
    if let Some(order) = self.orders.get_mut(&order_id) {
      
      let bid_or_ask = &order.bid_or_ask;
      //let parent_limit_price = order.parent_limit;
      let parent_price = order.parent_limit.expect("parent limit should exist here in modify limit!");
      let next_order_id = order.next_order;
      let prev_order_id = order.prev_order;
      let shares = order.shares;
      
      let (book_edge, tree, limit_map) = match bid_or_ask {
        BidOrAsk::Bid => (&mut self.highest_buy, &mut self.buy_tree, &mut self.buy_limits), 
        BidOrAsk::Ask => (&mut self.lowest_sell, &mut self.sell_tree, &mut self.sell_limits)
      };

      //order->cancel
      if let Some(prev_id) = prev_order_id {      
        if let Some(prev_order) = self.orders.get_mut(&prev_id) {
          prev_order.next_order = next_order_id;
        }
      } else {
        if let Some(parent_limit) = limit_map.get_mut(&parent_price) {
          parent_limit.head_order = next_order_id;
        }
      }

      if let Some(next_id) = next_order_id {
        if let Some(next_order) = self.orders.get_mut(&next_id) {
          next_order.prev_order = prev_order_id;
        }
      } else {
        if let Some(parent_limit) = limit_map.get_mut(&parent_price) {
          parent_limit.tail_order = prev_order_id;
        }
      }
    
      // order.cancel(limit_map, &mut self.orders);
      //order=>cancel over
      let parent_limit = limit_map.get_mut(&parent_price).expect("parent limit should exist here in modify!");
      parent_limit.total_volume -= shares;
      parent_limit.size -= 1;

      if parent_limit.size == 0 {
        delete_limit(&mut self.avl_rebalances,&parent_price, book_edge, tree, limit_map)
      }

      //CHECK IF IMMEDIATELY EXECUTABLE
      let b_or_a = self.orders.get(&order_id).expect("order should exist!").bid_or_ask.clone();
      let left_shares = self.limit_order_as_market_order(&order_id, &b_or_a, &mut new_shares, &new_limit_price);
      
      if left_shares == 0 {
        self.orders.remove(&order_id);
        self.limit_orders.remove(&order_id);
        are_rem_shares = false;
      } else { rem_shares = Some(left_shares); }

    };

    // modify order
    if let (true, Some(order)) = (are_rem_shares, self.orders.get_mut(&order_id)) {
      order.modify_order(rem_shares.expect("rem shares should exist here in modify order!"), new_limit_price);
      // check for new limit and insert
      let bid_or_ask = &order.bid_or_ask;
      
      match bid_or_ask {
        BidOrAsk::Bid => {
          if !self.buy_limits.contains_key(&new_limit_price) {
            self.add_limit(new_limit_price, BidOrAsk::Bid);
          }
          // append new order to new limit
          let nl = self.buy_limits.get_mut(&new_limit_price).unwrap();
          nl.append(order_id, &mut self.orders);
        },
        BidOrAsk::Ask => {
          if !self.sell_limits.contains_key(&new_limit_price) {
            self.add_limit(new_limit_price, BidOrAsk::Ask);
          }
          // append new order to new limit
          let nl = self.sell_limits.get_mut(&new_limit_price).unwrap();
          nl.append(order_id, &mut self.orders);
        }
      }
    }
  }

  pub fn cancel_limit_order(&mut self, order_id: u64) {
    
    self.avl_rebalances = 0;
    self.executed_orders_count = 0;
    
    // extract the order and cancel it
    if let Some(order) = self.orders.get_mut(&order_id) {
      
      let bid_or_ask = &order.bid_or_ask;
      let parent_price = order.parent_limit.expect("parent limit should exist here in modify limit!");
      let next_order_id = order.next_order;
      let prev_order_id = order.prev_order;
      let shares = order.shares;
      
      let (book_edge, tree, limit_map) = match bid_or_ask {
        BidOrAsk::Bid => (&mut self.highest_buy, &mut self.buy_tree, &mut self.buy_limits), 
        BidOrAsk::Ask => (&mut self.lowest_sell, &mut self.sell_tree, &mut self.sell_limits)
      };
      
      //order->cancel
      if let Some(prev_id) = prev_order_id {      
        if let Some(prev_order) = self.orders.get_mut(&prev_id) {
          prev_order.next_order = next_order_id;
        }
      } else {
        if let Some(parent_limit) = limit_map.get_mut(&parent_price) {
          parent_limit.head_order = next_order_id;
        }
      }

      if let Some(next_id) = next_order_id {
        if let Some(next_order) = self.orders.get_mut(&next_id) {
          next_order.prev_order = prev_order_id;
        }
      } else {
        if let Some(parent_limit) = limit_map.get_mut(&parent_price) {
          parent_limit.tail_order = prev_order_id;
        }
      }
    
      // order.cancel(limit_map, &mut self.orders);
      //order=>cancel over
      let parent_limit = limit_map.get_mut(&parent_price).expect("parent limit should exist here in modify!");
      parent_limit.total_volume -= shares;
      parent_limit.size -= 1;

      if parent_limit.size == 0 {
        delete_limit(&mut self.avl_rebalances,&parent_price, book_edge, tree, limit_map)
      }

      // deleteFromOrderMap(orderId)
      self.orders.remove(&order_id);
      self.limit_orders.remove(&order_id);
    }
  }

  fn add_limit(&mut self, limit_price: Decimal, bid_or_ask: BidOrAsk) {
    
    let (tree, book_edge, limit_map) = match bid_or_ask {
      BidOrAsk::Bid => {(&mut self.buy_tree, &mut self.highest_buy, &mut self.buy_limits)},
      BidOrAsk::Ask => {(&mut self.sell_tree, &mut self.lowest_sell, &mut self.sell_limits)}
    };

    let new_limit = Limit::new(limit_price, 0, 0, bid_or_ask.clone());
    limit_map.insert(limit_price,new_limit);

    match tree {
      None => {
        *tree = Some(limit_price);
        *book_edge = Some(limit_price);
      },
      Some(_) => {

        let _ = insert_recursive(&mut self.avl_rebalances, limit_map, tree, limit_price, None);
        // update bookedge i.e highestbuy/lowest sell
        match bid_or_ask {
          BidOrAsk::Bid => {
            if Some(limit_price) > *book_edge {
              *book_edge = Some(limit_price);
            }
          },
          BidOrAsk::Ask => {
            if Some(limit_price) < *book_edge {
              *book_edge = Some(limit_price);
            }
          }
        }
      }
    }
  }
}