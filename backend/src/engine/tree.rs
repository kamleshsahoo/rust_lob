use std::{cmp, collections::HashMap};
use rust_decimal::Decimal;
use super::orderbook::{BidOrAsk, Limit};

struct BinaryTree<'a> {
  limit_map: &'a mut HashMap<Decimal, Limit>,
  tree: Option<&'a mut Decimal>, // a.k.a the root of tree
  avl_rebalances: &'a mut u64
}

impl<'a> BinaryTree <'a> {

  fn to_limit(&self, limit_price: &Decimal) -> &Limit{
    self.limit_map.get(limit_price).expect("limit should exist when calling to_limit()!!")
  }

  fn new_for_insert(limit_map: &'a mut HashMap<Decimal, Limit>, avl_rebalances: &'a mut u64) -> Self {
    BinaryTree { limit_map, tree: None, avl_rebalances}
  } 

  fn new_for_delete(limit_map: &'a mut HashMap<Decimal, Limit>, tree: Option<&'a mut Decimal>, avl_rebalances: &'a mut u64) -> Self {
    BinaryTree { limit_map, tree, avl_rebalances}
  } 

  fn get_balance_factor(&self, limit: &Limit) -> i32 {

    let left_child_height = limit.left_child.map_or(0, |lc| self.limit_map.get(&lc).expect("left child should exist here in the closure of get balance factor!!").height);
    
    let right_child_height = limit.right_child.map_or(0, |rc| {if self.limit_map.get(&rc).is_none(){
      println!("[err] limit: {:?}", limit);
    }; self.limit_map.get(&rc).expect("right child should exist here in the closure of get balance factor!!").height});

    left_child_height - right_child_height
  }

  fn balance_tree(&mut self, limit_price: &Decimal) -> Decimal {

    let limit = self.limit_map.get(&limit_price).expect("limit should exist here when balancing in balance_tree()!!");
    let b_factor = self.get_balance_factor(limit);
  
    match b_factor {
      2.. => {
        let left_child = limit.left_child.expect("left child should exist here in balance_tree()!");
        let left_child_limit = self.to_limit(&left_child);
        let l: Decimal;

        if self.get_balance_factor(left_child_limit) >= 0 {
          l = self.ll_rotate(limit_price);
        } else {
          l = self.lr_rotate(limit_price);
        }
        *self.avl_rebalances += 1;
        l
      },
      ..-1 => {
        let right_child = limit.right_child.expect("right child should exist here in balance_tree()!");
        let right_child_limit = self.to_limit(&right_child);
        let l: Decimal;
  
        if self.get_balance_factor(right_child_limit) > 0 { 
          l = self.rl_rotate(limit_price);
        } else {
          l = self.rr_rotate(limit_price);
        }

        *self.avl_rebalances += 1;
        l
      },
      _ => {
        limit.limit_price
      }
    }
  }

  fn ll_rotate(&mut self, limit_price: &Decimal) -> Decimal {
            
    let new_parent_id = self.limit_map.get(limit_price).and_then(|parent| parent.left_child).expect("[LL] new parent id aka limit's left child should exist when getting new parent id!!");
    let new_parent_right_child_id = self.limit_map.get(&new_parent_id).and_then(|new_parent| new_parent.right_child);
    let parent_right_child_height = self.limit_map.get(limit_price).and_then(|parent| parent.right_child).map_or(0, |parent_rc| self.limit_map.get(&parent_rc).expect("[LL] parent right child should exist here in ll if the corresponding id is present!!").height);
    let parent_left_child_height = new_parent_right_child_id.map_or(0, |new_parent_rc| self.limit_map.get(&new_parent_rc).expect("[LL] new parent right child should exist here in ll if the corresponding id is present!!").height);
    
    // update (previous) parent attributes except parent's parent
    {
      let parent = self.limit_map.get_mut(limit_price).expect("[LL] parent limit should exist in ll rotate!!");
      parent.left_child = new_parent_right_child_id;
      parent.height = 1 + cmp::max(parent_left_child_height, parent_right_child_height);
    }

    if let Some(new_parent_right_child_price) = new_parent_right_child_id {
      let new_parent_right_child = self.limit_map.get_mut(&new_parent_right_child_price).expect("[LL] new parent right child should exist in ll if corresponding id/price exists here!!");
      new_parent_right_child.parent = Some(*limit_price);
    }

    let new_parent_right_child_height = self.limit_map.get(limit_price).expect("[LL] new parent right child i.e the limit price used to call ll rotate should exist!!").height;
    let new_parent_left_child_height = self.limit_map.get(&new_parent_id).and_then(|new_parent| new_parent.left_child).map_or(0, |new_parent_lc| self.limit_map.get(&new_parent_lc).expect("[LL] new parent left child should exist here in ll if corresponding id is present!!").height);
    let parent_parent = self.limit_map.get(limit_price).and_then(|parent| parent.parent);
    
    // update new parent attributes
    {
      let new_parent = self.limit_map.get_mut(&new_parent_id).expect("[LL] new parent limit should exist here in ll rotate!!");

      new_parent.right_child = Some(*limit_price);

      if let Some(parent_parent_id) = parent_parent {
        new_parent.parent = Some(parent_parent_id);
      } else {
        new_parent.parent = None;
        
        //TODO: check if overall tree/root needs to be set here
        // NOTE: below cond only runs when balancing done after deletes
        // insert rebalances have root tree intialized as None
        if let Some(tree_id) = self.tree.as_deref_mut() {
          *tree_id = new_parent_id;
        }
        
      }
      new_parent.height = 1 + cmp::max(new_parent_left_child_height, new_parent_right_child_height);
    }
    // update (previous) parent's parent
    {
      let parent = self.limit_map.get_mut(limit_price).expect("[LL] parent limit should exist in ll rotate!!");
      parent.parent = Some(new_parent_id);
    }

    new_parent_id
  }

  fn lr_rotate(&mut self, limit_price: &Decimal) -> Decimal {
    
    let new_parent_id = self.limit_map.get(limit_price).and_then(|parent| parent.left_child).expect("[LR] new parent id aka limit's left child should exist when getting new parent id!!");
    let temp_new_parent = self.rr_rotate(&new_parent_id);
    let parent = self.limit_map.get_mut(limit_price).expect("[LR] parent limit should exist in lr rotate (after performing rr)!!");
    parent.left_child = Some(temp_new_parent);

    self.ll_rotate(limit_price)
  }

  fn rr_rotate(&mut self, limit_price: &Decimal) -> Decimal {
  
    let new_parent_id = self.limit_map.get(limit_price).and_then(|parent| parent.right_child).expect("[RR] new parent id aka limit's right child should exist when getting new parent id!!");
    let new_parent_left_child_id = self.limit_map.get(&new_parent_id).and_then(|new_parent| new_parent.left_child);
    let parent_right_child_height = new_parent_left_child_id.map_or(0, |new_parent_lc| self.limit_map.get(&new_parent_lc).expect("[RR] new parent left child should exist here in rr if the corresponding id is present!!").height);
    let parent_left_child_height = self.limit_map.get(limit_price).and_then(|parent| parent.left_child).map_or(0, |parent_lc| self.limit_map.get(&parent_lc).expect("[RR] parent left child should exist here in rr if the corresponding id is present!!").height);
    
    // update (previous) parent attributes except parent's parent
    {
      let parent = self.limit_map.get_mut(limit_price).expect("[RR] parent limit should exist in rr rotate!!");

      parent.right_child = new_parent_left_child_id;

      parent.height = 1 + cmp::max(parent_left_child_height, parent_right_child_height);
    }

    if let Some(new_parent_left_child_price) = new_parent_left_child_id {

      let new_parent_left_child = self.limit_map.get_mut(&new_parent_left_child_price).expect("[RR] new parent left child should exist in rr if corresponding id/price exists here!!");
      
      new_parent_left_child.parent = Some(*limit_price);
    }

    let new_parent_right_child_height = self.limit_map.get(&new_parent_id).and_then(|new_parent| new_parent.right_child).map_or(0, |new_parent_rc| self.limit_map.get(&new_parent_rc).expect("[RR] new parent right child should exist here in rr if corresponding id is present!!").height);
    let new_parent_left_child_height = self.limit_map.get(limit_price).expect("[RR] new parent left child i.e the limit price used to call rr rotate should exist!!").height;
    let parent_parent = self.limit_map.get(limit_price).and_then(|parent| parent.parent);
    
    // update new parent attributes
    {
      let new_parent = self.limit_map.get_mut(&new_parent_id).expect("[RR] new parent limit should exist here in rr rotate!!");
      new_parent.left_child = Some(*limit_price);

      if let Some(parent_parent_id) = parent_parent {
        new_parent.parent = Some(parent_parent_id);
      } else {
        new_parent.parent = None;
        //TODO: check if overall tree/root needs to be set here
        // NOTE: below cond only runs when balancing done after deletes
        // insert rebalances have root tree intialized as None
        if let Some(tree_id) = self.tree.as_deref_mut() {
          *tree_id = new_parent_id;
        }
      }

      new_parent.height = 1 + cmp::max(new_parent_left_child_height, new_parent_right_child_height);
    }
    // update (previous) parent's parent
    {
      let parent = self.limit_map.get_mut(limit_price).expect("[RR] parent limit should exist in rr rotate!!");
      parent.parent = Some(new_parent_id);
    }

    new_parent_id
  }

  fn rl_rotate(&mut self, limit_price: &Decimal) -> Decimal {

    let new_parent_id = self.limit_map.get(limit_price).and_then(|parent| parent.right_child).expect("[RL] new parent id aka limit's right child should exist when getting new parent id!!");
    let temp_new_parent = self.ll_rotate(&new_parent_id);
    let parent = self.limit_map.get_mut(limit_price).expect("[RL] parent limit should exist in rl rotate (after performing ll)!!");

    parent.right_child = Some(temp_new_parent);

    self.rr_rotate(limit_price)
  }
} 

pub fn insert_recursive(avl_rebal_cnt: &mut u64, limit_map: &mut HashMap<Decimal, Limit>, root: &mut Option<Decimal>, limit_price: Decimal, parent: Option<Decimal>) -> Decimal{

  match root {
    None => {
      let limit = limit_map.get_mut(&limit_price).expect("limit should exist here in insert recursive!!");
      limit.parent = parent;
      limit_price
    },
    Some(root_price) => {
      if limit_price < *root_price {
        let mut left_child = limit_map.get_mut(&root_price).expect("root price limit should exist here!!").left_child;
        let result = insert_recursive(avl_rebal_cnt,limit_map, &mut left_child, limit_price, Some(*root_price));
        limit_map.get_mut(&root_price).expect("root price limit should exist here!!").left_child = Some(result);
      }
      else if limit_price > *root_price {
        let mut right_child = limit_map.get_mut(&root_price).expect("root price limit should exist here!!").right_child;
        let result = insert_recursive(avl_rebal_cnt, limit_map, &mut right_child, limit_price, Some(*root_price));
        limit_map.get_mut(&root_price).expect("root price limit should exist here!!").right_child = Some(result);
      }
      // TODO: check if need to handle limit_price == root_price case
      
      // update root height  
      let left_height = limit_map
      .get(root_price)
      .and_then(|root| root.left_child.and_then(|lc| limit_map.get(&lc)))
      .map_or(0, |child| child.height);
      
      let right_height = limit_map
      .get(root_price)
      .and_then(|root| root.right_child.and_then(|lc| limit_map.get(&lc)))
      .map_or(0, |child| child.height);

      let root_limit = limit_map.get_mut(root_price).expect("root limit should exist in insert recursive()!!");
      root_limit.height = 1 + cmp::max(left_height, right_height);
        
      //TODO: AVL balancing
      // NOTE: below 'tree' arg is not needed to be passed since we already balance the tree using the 'root_price'
      let mut bst = BinaryTree::new_for_insert(limit_map, avl_rebal_cnt);
      *root_price = bst.balance_tree(root_price);

      // finally return the root price
      *root_price
    }
  }
}

pub fn delete_limit(avl_rebal_cnt: &mut u64, limit_price: &Decimal, book_edge: &mut Option<Decimal>, tree: &mut Option<Decimal>, limit_map: &mut HashMap<Decimal, Limit>) {
    
  // update bookedge if the removed limit was bookedge
  if Some(*limit_price) == *book_edge {
    
    let bid_or_ask = &limit_map.get(limit_price).unwrap().bid_or_ask;
    let book_edge_l = limit_map.get(&book_edge.unwrap()).unwrap();
    
    if book_edge != tree {
      match bid_or_ask {
        BidOrAsk::Bid if book_edge_l.left_child.is_some() => *book_edge = book_edge_l.left_child,
        BidOrAsk::Ask if book_edge_l.right_child.is_some() => *book_edge = book_edge_l.right_child,
        _ => *book_edge = book_edge_l.parent
      }
    }
    else {
      match bid_or_ask {
        BidOrAsk::Bid if book_edge_l.left_child.is_some() => *book_edge = book_edge_l.left_child,
        BidOrAsk::Ask if book_edge_l.right_child.is_some() => *book_edge = book_edge_l.right_child,
        _ => *book_edge = None
      }
    }
  }
  
  // delete from limit map
  let limit = limit_map.remove(limit_price).unwrap();
  let tree_price = tree.expect("tree should exist here in delete limit");
  
  if tree_price == *limit_price {
    
    if limit.right_child.is_some() {
      *tree = limit.right_child;
      let mut tree_l = limit_map.get(&tree.unwrap()).unwrap();
      
      while let Some(tree_left_child_id) = tree_l.left_child {
        tree_l = limit_map.get(&tree_left_child_id).unwrap();
      }
      *tree = Some(tree_l.limit_price);
    } else {
      *tree = limit.left_child;
    }
  }

  //handle side effects of deleting limit (linking correct nodes)
  handle_delete_limit(&limit, limit_map);

  //setting parents
  let mut parent_limit_price = limit.parent;

  while parent_limit_price.is_some() {

    let mut parent_limit_price_unwraped = parent_limit_price.expect("parent limit price cannot be None since already checked in the above while exp");
    // balance AVL tree
    let mut bst = BinaryTree::new_for_delete(limit_map, tree.as_mut(), avl_rebal_cnt);
    parent_limit_price_unwraped = bst.balance_tree(&parent_limit_price_unwraped);

    if let Some(parent_limit_) = limit_map.get(&parent_limit_price_unwraped) {
      if parent_limit_.parent.is_some(){ 
        if let Some(x) = limit_map.get_mut(&parent_limit_.parent.expect("parent of the parent limit price should exist, since already checked in the above if exp")) {
          if x.limit_price > limit.limit_price { 
            x.left_child = Some(parent_limit_price_unwraped); 
          } else {
            x.right_child = Some(parent_limit_price_unwraped);
          }
        }
      }
    }
    parent_limit_price = limit_map.get(&parent_limit_price_unwraped).unwrap().parent;
  }
}

fn handle_delete_limit(limit: &Limit, limit_map: &mut HashMap<Decimal, Limit>) {

  match limit.parent {
    Some(parent_price) => {

      let mut parent_limit = limit_map.remove(&parent_price).expect("parent limit should exist here in delete_limit()!!");
      let left_or_right_child = limit.limit_price < parent_price;

      // Case 1: Node with 1 or no child
      if limit.left_child.is_none() {
        
        if left_or_right_child {
          parent_limit.left_child = limit.right_child;
        } else {
          parent_limit.right_child = limit.right_child;
        }

        if limit.right_child.is_some() {
          let right_child_id = limit.right_child.expect("right child id(or price) should exist here in delete_limit()!!");
          let right_child_limit = limit_map.get_mut(&right_child_id).expect("right child limit should exist here in delete_limit()!!");
          right_child_limit.parent = limit.parent;
        }
      // NOTE: adding back parent limit before return
      limit_map.insert(parent_price, parent_limit);
      return
      } else if limit.right_child.is_none() {
    
        if left_or_right_child {
          parent_limit.left_child = limit.left_child;
        } else {
          parent_limit.right_child = limit.left_child;
        }

        let left_child_limit = limit_map.get_mut(&limit.left_child.expect("left child id(or price) should exist here in delete_limit()!!")).expect("left child limit should exist here in delete_limit()!!");
        left_child_limit.parent = limit.parent;
        
        // NOTE: adding back parent limit before return
        limit_map.insert(parent_price, parent_limit);
        return
      }

      // Case 2: Node with two children
      let mut temp_id = limit.right_child.expect("right child id(or price) should exist here in delete_limit()!!");

      find_leftmost(limit_map, &mut temp_id);
      let right_child =  limit_map.get(&limit.right_child.expect("right child should exist here ...!!")).expect("right limit should exist here...!!");

      if right_child.left_child.is_some() {

        let mut temp = limit_map.remove(&temp_id).expect("temp limit should exist here after find_leftmost() above!!");
        let temp_parent_id = temp.parent.expect("temp must have parent here!!");
        let temp_parent = limit_map.get_mut(&temp_parent_id).expect("temp should have parent limit here!!");

        temp_parent.left_child = temp.right_child;

        if temp.right_child.is_some() {
          let temp_right_child_id = temp.right_child.expect("right child of temp should exist here!!");
          let temp_right_child = limit_map.get_mut(&temp_right_child_id).expect("temp should have right child here!!");
          temp_right_child.parent = temp.parent;
        }

        temp.right_child = limit.right_child;
        let right_child =  limit_map.get_mut(&limit.right_child.expect("right child should exist here ...!!")).expect("right limit should exist here...!!");

        right_child.parent = Some(temp.limit_price);
        limit_map.insert(temp_id, temp);

      }

      // temp->setParent(parent)
      let temp = limit_map.get_mut(&temp_id).expect("temp limit should exist here!!");

      temp.parent = Some(parent_price);
      temp.left_child = limit.left_child;

      let left_child_id = limit.left_child.expect("left child id should exist here..");
      let left_child = limit_map.get_mut(&left_child_id).expect("left child limit should exist here!!");
      left_child.parent = Some(temp_id);

      if left_or_right_child {
        parent_limit.left_child = Some(temp_id);
      } else {
        parent_limit.right_child = Some(temp_id);
      }

      //NOTE: **Dont forget to insert back the removed parent**
      limit_map.insert(parent_price, parent_limit);
    },
    None => {
      // Case 1: Node with 1 or no child
      if limit.left_child.is_none() && limit.right_child.is_none() {
        return
      } else if limit.left_child.is_none() {
          let right_child_limit = limit_map.get_mut(&limit.right_child.expect("right child id(or price) should exist here in delete_limit() - without parent case!!")).expect("right child should exist here in delete_limit() - without parent case!!");
          right_child_limit.parent = None;
          return
      } else if limit.right_child.is_none() {
        let left_child_limit = limit_map.get_mut(&limit.left_child.expect("left child id(or price) should exist here in delete_limit() - without parent case!!")).expect("left child should exist here in delete_limit() - without parent case!!");
        left_child_limit.parent = None;
        return
      }

      // Case 2: Node with 2 children
      let mut temp_id = limit.right_child.expect("right child id(or price) should exist here in delete_limit() None arm!!");

      find_leftmost(limit_map, &mut temp_id);

      let right_child =  limit_map.get(&limit.right_child.expect("right child should exist here ...!!")).expect("right limit should exist here...!!");

      if right_child.left_child.is_some() {

        let mut temp = limit_map.remove(&temp_id).expect("temp limit should exist here after find_leftmost() above!!");
        let temp_parent_id = temp.parent.expect("temp must have parent here!!");
        let temp_parent = limit_map.get_mut(&temp_parent_id).expect("temp should have parent limit here!!");

        temp_parent.left_child = temp.right_child;

        if temp.right_child.is_some() {
          let temp_right_child_id = temp.right_child.expect("right child of temp should exist here!!");
          let temp_right_child = limit_map.get_mut(&temp_right_child_id).expect("temp should have right child here!!");
          temp_right_child.parent = temp.parent;
        }

        temp.right_child = limit.right_child;
        let right_child =  limit_map.get_mut(&limit.right_child.expect("right child should exist here ...!!")).expect("right limit should exist here...!!");
        right_child.parent = Some(temp.limit_price);
        limit_map.insert(temp_id, temp);
      }

      let temp = limit_map.get_mut(&temp_id).expect("temp limit should exist here!!");
      temp.parent = None;
      temp.left_child = limit.left_child;

      let left_child_id = limit.left_child.expect("left child id should exist here..");
      let left_child = limit_map.get_mut(&left_child_id).expect("left child limit should exist here!!");
      left_child.parent = Some(temp_id);
    }
  } 
}

fn find_leftmost(limit_map: & HashMap<Decimal, Limit>, node_id: &mut Decimal) {
  while let Some(left_child_id) = limit_map.get(&node_id).expect("node id should exist in find leftmost()!!").left_child {
    *node_id = left_child_id
  }
}