use bson::Bson;
use dustdata::DustData;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::config;
use crate::query;
use crate::server;
use crate::server::wirewave::server::ResHeader;

use config::schema;
use server::cache;
use server::wirewave;

use cache::Cache;
use query::parser::{ASTNode, Keywords, Verbs};
use wirewave::authorization::UserPermission;
use wirewave::server::{Error, Response, Status};

use interface::TransactionError;

use super::interface;

pub struct Core {
    interface: interface::DustDataInterface,
}

impl Core {
    pub fn new(
        cache: Arc<RwLock<Cache>>,
        routers: Arc<RwLock<HashMap<String, DustData>>>,
        config: Arc<schema::RustbaseConfig>,
        system_db: Arc<RwLock<DustData>>,
        current_database: String,
        current_user: Option<String>,
    ) -> Self {
        let interface = interface::DustDataInterface::new(
            cache,
            routers,
            config,
            system_db,
            current_database,
            current_user,
        );

        Self { interface }
    }

    /// `run_ast` takes an ASTNode and returns a Result<Response, Status>
    ///
    /// Arguments:
    ///
    /// * `ast`: The ASTNode that is being run.
    ///
    /// Returns:
    ///
    /// A Result<Response, Status>
    pub fn run_ast(&mut self, ast: ASTNode) -> Result<Response, Error> {
        match ast {
            ASTNode::IntoExpression {
                keyword,
                json,
                ident,
            } => self.expr_into(keyword, *json, *ident),

            ASTNode::MonadicExpression {
                keyword,
                verb,
                expr,
            } => self.monadic_expr(keyword, verb, expr),

            ASTNode::SingleExpression { keyword, ident } => self.sgl_expr(keyword, ident),
            _ => {
                let error = Error {
                    message: "Invalid query".to_string(),
                    query_message: None,
                    status: Status::InvalidQuery,
                };

                Err(error)
            }
        }
    }

    /// `expr_into` is a function that takes a keyword, a value, and an expression, and returns a response
    /// or a status
    ///
    /// Arguments:
    ///
    /// * `keyword`: The keyword that was used in the query.
    /// * `value`: The value to be inserted or updated.
    /// * `expr`: The expression to be evaluated.
    ///
    /// Returns:
    ///
    /// A response or a status.
    fn expr_into(
        &mut self,
        keyword: Keywords,
        value: ASTNode,
        expr: ASTNode,
    ) -> Result<Response, Error> {
        match keyword {
            Keywords::Insert => self.ast_into_insert(value, expr),

            Keywords::Update => self.ast_into_update(value, expr),

            _ => {
                let error = Error {
                    message: format!("{:?} is unexpected for into expression", keyword),
                    query_message: None,
                    status: Status::InvalidQuery,
                };

                Err(error)
            }
        }
    }

    /// It takes a keyword, a verb, and an optional expression, and then it matches on the keyword and verb
    /// to determine which function to call
    ///
    /// Arguments:
    ///
    /// * `keyword`: The keyword that the user is using.
    /// * `verb`: The verb of the query.
    /// * `expr`: Option<Vec<ASTNode>>
    ///
    /// Returns:
    ///
    /// A response or a status.
    fn monadic_expr(
        &mut self,
        keyword: Keywords,
        verb: Verbs,
        expr: Option<Vec<ASTNode>>,
    ) -> Result<Response, Error> {
        match keyword {
            Keywords::Insert => match verb {
                Verbs::User => self.ast_user_insert(expr),

                _ => {
                    let error = Error {
                        message: format!("{:?} is unexpected for insert expression", verb),
                        query_message: None,
                        status: Status::InvalidQuery,
                    };

                    Err(error)
                }
            },

            Keywords::Delete => match verb {
                Verbs::Database => self.ast_database_delete(expr),

                Verbs::User => self.ast_user_delete(expr),
            },

            Keywords::Update => match verb {
                Verbs::User => self.ast_user_update(expr),

                _ => {
                    let error = Error {
                        message: format!("{:?} is unexpected for update expression", verb),
                        query_message: None,
                        status: Status::InvalidQuery,
                    };

                    Err(error)
                }
            },

            _ => {
                let error = Error {
                    message: format!("{:?} is unexpected for monadic expression", keyword),
                    query_message: None,
                    status: Status::InvalidQuery,
                };

                Err(error)
            }
        }
    }

    /// It takes a keyword and an identifier, and returns a response
    ///
    /// Arguments:
    ///
    /// * `keyword`: The keyword that was used to start the query.
    /// * `ident`: The identifier of the object to be operated on.
    ///
    /// Returns:
    ///
    /// A response object.
    fn sgl_expr(
        &mut self,
        keyword: Keywords,
        ident: Option<Box<ASTNode>>,
    ) -> Result<Response, Error> {
        match keyword {
            Keywords::Get => self.ast_sgl_get(ident),

            Keywords::Delete => self.ast_sgl_delete(ident),

            Keywords::List => self.ast_sgl_list(),

            _ => {
                let error = Error {
                    message: format!("{:?} is unexpected for single expression", keyword),
                    query_message: None,
                    status: Status::InvalidQuery,
                };

                Err(error)
            }
        }
    }

    /// It takes a key and a value, and inserts the value into the database
    ///
    /// Arguments:
    ///
    /// * `value`: The value to insert into the database.
    /// * `expr`: The expression that is being evaluated.
    ///
    /// Returns:
    ///
    /// A response object.
    fn ast_into_insert(&mut self, value: ASTNode, expr: ASTNode) -> Result<Response, Error> {
        let key = match expr {
            ASTNode::Identifier(ident) => ident,
            _ => return query_error("key must be an identifier"),
        };

        let value = match value {
            ASTNode::Bson(json) => json,
            _ => return query_error("value must be a json object"),
        };

        match self.interface.insert_into_dustdata(key, value) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// It takes an ASTNode and an ASTNode, and returns a Result<Response, Error>
    ///
    /// Arguments:
    ///
    /// * `value`: The value to update the key with.
    /// * `expr`: The expression to evaluate.
    ///
    /// Returns:
    ///
    /// A response object.
    fn ast_into_update(&mut self, value: ASTNode, expr: ASTNode) -> Result<Response, Error> {
        let key = match expr {
            ASTNode::Identifier(ident) => ident,
            _ => return query_error("key must be an identifier"),
        };

        let value = match value {
            ASTNode::Bson(json) => json,
            _ => return query_error("value must be a json object"),
        };

        match self.interface.update_dustdata(key, value) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// It takes a `Vec<ASTNode>` and returns a `Result<Response, Error>`
    ///
    /// Arguments:
    ///
    /// * `expr`: Option<Vec<ASTNode>>
    ///
    /// Returns:
    ///
    /// A response object
    fn ast_user_insert(&mut self, expr: Option<Vec<ASTNode>>) -> Result<Response, Error> {
        if expr.is_none() {
            return query_error("user insert must have an expression");
        }

        let expr = expr.unwrap();

        let mut username = String::new();
        let mut permission = String::new();
        let mut password = String::new();

        // idk if this is the best way to do this
        for node in expr {
            match node {
                // this will find the password and permission
                ASTNode::AssignmentExpression { ident, value } => {
                    match ident.as_str() {
                        "password" => {
                            password = match *value {
                                ASTNode::Bson(s) => {
                                    let s = s.as_str();

                                    // if the password is not a string, return an error
                                    if let Some(s) = s {
                                        s.to_string()
                                    } else {
                                        return query_error("password must be a string");
                                    }
                                }

                                _ => {
                                    return query_error("password must be a string");
                                }
                            }
                        }

                        "permission" => {
                            permission = match *value {
                                ASTNode::Bson(s) => {
                                    let s = s.as_str();

                                    // if the permission is not a string, return an error
                                    if let Some(s) = s {
                                        s.to_string()
                                    } else {
                                        return query_error("permission must be a string");
                                    }
                                }
                                _ => {
                                    return query_error("permission must be a string");
                                }
                            }
                        }

                        _ => {}
                    }
                }

                ASTNode::Identifier(ref ident) => username = ident.clone(),

                _ => {}
            }
        }

        if username.is_empty() || password.is_empty() || permission.is_empty() {
            return query_error("username, password, and permission are required");
        }

        let permission = UserPermission::from_str(permission.as_str());

        if permission.is_err() {
            return query_error(
                "permission must be 'read' or 'write', 'read_and_write', or 'admin'",
            );
        }

        match self
            .interface
            .create_user(username, password, permission.unwrap())
        {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// `ast_user_delete` is a function that takes a `Option<Vec<ASTNode>>` and returns a `Result<Response,
    /// Status>`
    ///
    /// Arguments:
    ///
    /// * `expr`: The expression that was passed to the command.
    ///
    /// Returns:
    ///
    /// A `Result` type.
    fn ast_user_delete(&mut self, expr: Option<Vec<ASTNode>>) -> Result<Response, Error> {
        let user = if let Some(expr) = expr {
            match expr[0] {
                ASTNode::Identifier(ref ident) => ident.clone(),
                _ => {
                    return query_error("user delete must have an expression");
                }
            }
        } else {
            return query_error("user delete must have an expression");
        };

        match self.interface.delete_user(user) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// It takes a vector of ASTNodes, and if the vector is not empty, it will iterate through the
    /// vector, and if the ASTNode is an AssignmentExpression, it will check if the ident is "password"
    /// or "permission", and if it is, it will check if the value is a Bson, and if it is, it will check
    /// if the Bson is a string, and if it is, it will set the password or permission to the string
    ///
    /// Arguments:
    ///
    /// * `expr`: The ASTNode that represents the expression.
    ///
    /// Returns:
    ///
    /// A response object.
    fn ast_user_update(&mut self, expr: Option<Vec<ASTNode>>) -> Result<Response, Error> {
        if expr.is_none() {
            return query_error("user update must have an expression");
        }

        let mut password: Option<String> = None;
        let mut permission: Option<String> = None;
        let mut username = String::new();

        for node in expr.unwrap() {
            match node {
                // this will find the password and permission
                ASTNode::AssignmentExpression { ident, value } => {
                    match ident.as_str() {
                        "password" => {
                            password = match *value {
                                ASTNode::Bson(s) => {
                                    let s = s.as_str();

                                    // if the password is not a string, return an error
                                    if let Some(s) = s {
                                        Some(s.to_string())
                                    } else {
                                        return query_error("password must be a string");
                                    }
                                }
                                _ => None,
                            }
                        }

                        "permission" => {
                            permission = match *value {
                                ASTNode::Bson(s) => {
                                    let s = s.as_str();

                                    // if the password is not a string, return an error
                                    if let Some(s) = s {
                                        Some(s.to_string())
                                    } else {
                                        return query_error("permission must be a string");
                                    }
                                }
                                _ => None,
                            }
                        }

                        _ => {}
                    }
                }

                ASTNode::Identifier(ref ident) => username = ident.clone(),

                _ => {}
            }
        }

        let permission = if let Some(permission) = permission {
            let permission = UserPermission::from_str(permission.as_str());
            if permission.is_err() {
                return query_error(
                    "permission must be 'read' or 'write', 'read_and_write', or 'admin'",
                );
            };

            Some(permission.unwrap())
        } else {
            None
        };

        match self.interface.update_user(username, password, permission) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// `if the user provided a database name, use it, otherwise use the current database`
    ///
    /// Arguments:
    ///
    /// * `expr`: The expression that was passed to the function.
    ///
    /// Returns:
    ///
    /// A response object.
    fn ast_database_delete(&mut self, expr: Option<Vec<ASTNode>>) -> Result<Response, Error> {
        let database = if let Some(expr) = expr {
            match expr[0] {
                ASTNode::Identifier(ref ident) => ident.clone(),
                _ => {
                    unreachable!()
                }
            }
        } else {
            self.interface.current_database.clone()
        };

        match self.interface.delete_database(database) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// It gets a value from the database.
    ///
    /// Arguments:
    ///
    /// * `ident`: The identifier of the key to get.
    ///
    /// Returns:
    ///
    /// A `Result` type.
    fn ast_sgl_get(&mut self, ident: Option<Box<ASTNode>>) -> Result<Response, Error> {
        if ident.is_none() {
            return query_error("get must have an expression");
        }

        let key = match *ident.unwrap() {
            ASTNode::Identifier(ident) => ident,
            _ => {
                unreachable!()
            }
        };

        match self.interface.get_from_dustdata(key) {
            Ok(value) => Ok(Response {
                body: Some(value),
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// It deletes a key from the database.
    ///
    /// Arguments:
    ///
    /// * `ident`: The identifier of the key to delete.
    ///
    /// Returns:
    ///
    /// A response object.
    fn ast_sgl_delete(&mut self, ident: Option<Box<ASTNode>>) -> Result<Response, Error> {
        let key = match *ident.unwrap() {
            ASTNode::Identifier(ident) => ident,
            _ => {
                unreachable!()
            }
        };

        match self.interface.delete_from_dustdata(key) {
            Ok(_) => Ok(Response {
                body: None,
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    /// `fn ast_sgl_list(&self, ident: Option<Box<ASTNode>>) -> Result<Response, Status>`
    ///
    /// The function name is `ast_sgl_list` and it takes two arguments: `&self` and `ident:
    /// Option<Box<ASTNode>>`. The return type is `Result<Response, Status>`
    ///
    /// Arguments:
    ///
    /// * `ident`: The identifier of the node.
    ///
    /// Returns:
    ///
    /// A `Response` object.
    fn ast_sgl_list(&mut self) -> Result<Response, Error> {
        match self.interface.list_from_dustdata() {
            Ok(keys) => Ok(Response {
                body: Some(Bson::Array(keys.into_iter().map(Bson::String).collect())),
                header: ResHeader {
                    is_error: false,
                    messages: None,
                    status: Status::Ok,
                },
            }),

            Err(e) => self.dd_error(e),
        }
    }

    // error
    fn dd_error(&self, error: TransactionError) -> Result<Response, Error> {
        match error {
            TransactionError::InternalError(e) => {
                let code = parse_dd_error_code(e.code);

                Err(Error {
                    message: code.1,
                    status: code.0,
                    query_message: None,
                })
            }
            TransactionError::ExternalError(e, message) => Err(Error {
                message,
                status: e,
                query_message: None,
            }),
        }
    }
}

fn parse_dd_error_code(code: dustdata::ErrorCode) -> (Status, String) {
    match code {
        dustdata::ErrorCode::KeyExists => (Status::AlreadyExists, "key already exists".to_string()),
        dustdata::ErrorCode::KeyNotExists => (Status::AlreadyExists, "key not exists".to_string()),
        dustdata::ErrorCode::NotFound => (Status::NotFound, "not found".to_string()),
    }
}

fn query_error(msg: &str) -> Result<Response, Error> {
    Err(Error {
        message: msg.to_string(),
        status: Status::InvalidQuery,
        query_message: None,
    })
}
