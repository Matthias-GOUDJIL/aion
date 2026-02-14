#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Mots-clés du système
    Fn, Let, Mut, Struct, Enum, Return, If, Else, Match,
    Use, Pub, Async, Unsafe, Require,
    Interface, Impl, Channel, // Nouveau : Canaux
    
    // SPÉCIAL AION : L'IA et la Logique
    Intent, 
    Invariant, // Nouveau : Règles dures
    Inside, 

    // Identifiants et Littéraux
    Identifier(String),
    StringLiteral(String),
    IntLiteral(i64),
    FloatLiteral(f64),

    // Symboles et Opérateurs
    Plus, Minus, Star, Slash, 
    Eq, EqEq, NotEq, Arrow, 
    LArrow, // Nouveau : <- (Send/Receive)
    Gt, Lt, GtEq, LtEq,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket, 
    Colon, Semicolon, Comma, Dot, 
    DoubleColon, 
    Range,       
    At,
    Question,

    // Fin de fichier
    EOF,
    
    // Erreur
    Illegal(char),
}
