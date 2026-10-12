//! Port of `KotlinParsing.java` lines 266-544 (preamble, imports, top-level declarations).

use ktrs_syntax::SyntaxKind::{self, *};

use super::parameters::{AnnotationParsingMode, ModifierDetector};
use super::properties::DeclarationParsingMode;
use super::{IMPORT_RECOVERY_SET, NameParsingMode, PACKAGE_NAME_RECOVERY_SET, SEMICOLON_SET};
use crate::builder::{EdgeBinder, Marker};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /*
     *preamble
     *  : fileAnnotationList? packageDirective? import*
     *  ;
     */
    pub(super) fn parse_preamble(&mut self) {
        let first_entry = self.mark();

        /*
         * fileAnnotationList
         *   : fileAnnotations*
         */
        self.parse_file_annotation_list(AnnotationParsingMode::FileAnnotationsBeforePackage);

        /*
         * packageDirective
         *   : modifiers "package" SimpleName{"."} SEMI?
         *   ;
         */
        let mut package_directive = self.mark();
        self.parse_modifier_list(TokenSet::EMPTY);

        if self.at(PACKAGE_KEYWORD) {
            self.advance(); // PACKAGE_KEYWORD

            self.parse_package_name();

            first_entry.drop(self);

            self.consume_if(SEMICOLON);

            package_directive.done(self, PACKAGE_DIRECTIVE);
        } else {
            // When package directive is omitted we should not report error on non-file annotations at the beginning of the file.
            // So, we rollback the parsing position and reparse file annotation list without report error on non-file annotations.
            first_entry.rollback_to(self);

            self.parse_file_annotation_list(AnnotationParsingMode::FileAnnotationsWhenPackageOmitted);
            package_directive = self.mark();
            package_directive.done(self, PACKAGE_DIRECTIVE);
            // Need to skip everything but shebang comment to allow comments at the start of the file to be bound to the first declaration.
            package_directive.set_custom_edge_token_binders(self, Some(EdgeBinder::BindFirstShebangWithWhitespaceOnly), None);
        }

        self.parse_import_directives();
    }

    /* SimpleName{"."} */
    fn parse_package_name(&mut self) {
        let mut qualified_expression = self.mark();
        let mut simple_name = true;
        loop {
            if self.my_builder.newline_before_current_token() {
                self.error_with_recovery(
                    "Package name must be a '.'-separated identifier list placed on a single line",
                    Some(PACKAGE_NAME_RECOVERY_SET),
                );
                break;
            }

            if self.at(DOT) {
                self.advance(); // DOT
                qualified_expression.error(self, "Package name must be a '.'-separated identifier list");
                qualified_expression = self.mark();
                continue;
            }

            let ns_name = self.mark();
            let simple_name_found = self.expect_3(
                IDENTIFIER,
                "Package name must be a '.'-separated identifier list",
                Some(PACKAGE_NAME_RECOVERY_SET),
            );
            if simple_name_found {
                ns_name.done(self, REFERENCE_EXPRESSION);
            } else {
                ns_name.drop(self);
            }

            if !simple_name {
                let preceding_marker = qualified_expression.precede(self);
                qualified_expression.done(self, DOT_QUALIFIED_EXPRESSION);
                qualified_expression = preceding_marker;
            }

            if self.at(DOT) {
                self.advance(); // DOT

                if simple_name && !simple_name_found {
                    qualified_expression.drop(self);
                    qualified_expression = self.mark();
                } else {
                    simple_name = false;
                }
            } else {
                break;
            }
        }
        qualified_expression.drop(self);
    }

    /*
     * import
     *   : "import" SimpleName{"."} ("." "*" | "as" SimpleName)? SEMI?
     *   ;
     */
    fn parse_import_directive(&mut self) {
        debug_assert!(self._at(IMPORT_KEYWORD));
        let import_directive = self.mark();
        self.advance(); // IMPORT_KEYWORD

        if self.close_import_with_error_if_newline(import_directive, None, "Expecting qualified name") {
            return;
        }

        if !self.at(IDENTIFIER) {
            let error = self.mark();
            self.skip_until(TokenSet::create(&[EOL_OR_SEMICOLON]));
            error.error(self, "Expecting qualified name");
            import_directive.done(self, IMPORT_DIRECTIVE);
            self.consume_if(SEMICOLON);
            return;
        }

        let mut qualified_name = self.mark();
        let mut reference = self.mark();
        self.advance(); // IDENTIFIER
        reference.done(self, REFERENCE_EXPRESSION);

        while self.at(DOT) && self.lookahead(1) != Some(MUL) {
            self.advance(); // DOT

            if self.close_import_with_error_if_newline(import_directive, None, "Import must be placed on a single line") {
                qualified_name.drop(self);
                return;
            }

            reference = self.mark();
            if self.expect_3(IDENTIFIER, "Qualified name must be a '.'-separated identifier list", Some(IMPORT_RECOVERY_SET)) {
                reference.done(self, REFERENCE_EXPRESSION);
            } else {
                reference.drop(self);
            }

            let precede = qualified_name.precede(self);
            qualified_name.done(self, DOT_QUALIFIED_EXPRESSION);
            qualified_name = precede;
        }
        qualified_name.drop(self);

        if self.at(DOT) {
            self.advance(); // DOT
            debug_assert!(self._at(MUL));
            self.advance(); // MUL
            if self.at(AS_KEYWORD) {
                let as_ = self.mark();
                self.advance(); // AS_KEYWORD
                if self.close_import_with_error_if_newline(import_directive, None, "Expecting identifier") {
                    as_.drop(self);
                    return;
                }
                self.consume_if(IDENTIFIER);
                as_.done(self, IMPORT_ALIAS);
                let error = as_.precede(self);
                error.error(self, "Cannot rename all imported items to one identifier");
            }
        }
        if self.at(AS_KEYWORD) {
            let alias = self.mark();
            self.advance(); // AS_KEYWORD
            if self.close_import_with_error_if_newline(import_directive, Some(alias), "Expecting identifier") {
                return;
            }
            self.expect_3(IDENTIFIER, "Expecting identifier", Some(SEMICOLON_SET));
            alias.done(self, IMPORT_ALIAS);
        }
        self.consume_if(SEMICOLON);
        import_directive.done(self, IMPORT_DIRECTIVE);
        import_directive.set_custom_edge_token_binders(self, None, Some(EdgeBinder::TrailingComments));
    }

    fn close_import_with_error_if_newline(
        &mut self,
        import_directive: Marker,
        import_alias: Option<Marker>,
        error_message: &str,
    ) -> bool {
        if self.my_builder.newline_before_current_token() {
            if let Some(import_alias) = import_alias {
                import_alias.done(self, IMPORT_ALIAS);
            }
            self.error(error_message);
            import_directive.done(self, IMPORT_DIRECTIVE);
            return true;
        }
        false
    }

    fn parse_import_directives(&mut self) {
        let import_list = self.mark();
        if !self.at(IMPORT_KEYWORD) {
            // this is necessary to allow comments at the start of the file to be bound to the first declaration
            import_list.set_custom_edge_token_binders(self, Some(EdgeBinder::DoNotBindAnything), None);
        }
        while self.at(IMPORT_KEYWORD) {
            self.parse_import_directive();
        }
        import_list.done(self, IMPORT_LIST);
    }

    /*
     * toplevelObject
     *   : package
     *   : class
     *   : extension
     *   : function
     *   : property
     *   : typeAlias
     *   : object
     *   ;
     */
    pub(super) fn parse_top_level_declaration(&mut self) {
        if self.at(SEMICOLON) {
            self.advance(); // SEMICOLON
            return;
        }
        let decl = self.mark();

        let mut detector = ModifierDetector::default();
        self.parse_modifier_list_3(Some(&mut detector), TokenSet::EMPTY, /* localDeclaration = */ false);

        let mut decl_type =
            self.parse_common_declaration(&detector, NameParsingMode::Required, DeclarationParsingMode::MemberOrToplevel);

        if decl_type.is_none() && self.at(LBRACE) {
            self.error("Expecting a top level declaration");
            self.parse_block();
            decl_type = Some(FUN);
        }

        if decl_type.is_none() && self.at(IMPORT_KEYWORD) {
            self.error("imports are only allowed in the beginning of file");
            self.parse_import_directives();
            decl.drop(self);
        } else if let Some(decl_type) = decl_type {
            self.close_declaration_with_comment_binders(decl, decl_type, true);
        } else {
            self.error_and_advance("Expecting a top level declaration");
            decl.drop(self);
        }
    }

    pub(crate) fn parse_common_declaration(
        &mut self,
        detector: &ModifierDetector,
        name_parsing_mode_for_object: NameParsingMode,
        declaration_parsing_mode: DeclarationParsingMode,
    ) -> Option<SyntaxKind> {
        match self.get_token_id() {
            Some(CLASS_KEYWORD | INTERFACE_KEYWORD) => return Some(self.parse_class(detector.is_enum_detected(), true)),
            Some(FUN_KEYWORD) => return Some(self.parse_function()),
            Some(VAL_KEYWORD | VAR_KEYWORD) => return Some(self.parse_property(declaration_parsing_mode)),
            Some(LPAR | LBRACKET) => {
                let lookahead = self.lookahead(1);
                return if lookahead == Some(VAL_KEYWORD) || lookahead == Some(VAR_KEYWORD) {
                    Some(self.parse_property(declaration_parsing_mode))
                } else {
                    None
                };
            }
            Some(TYPE_ALIAS_KEYWORD) => return Some(self.parse_type_alias()),
            Some(OBJECT_KEYWORD) => {
                self.parse_object(name_parsing_mode_for_object, true);
                return Some(OBJECT_DECLARATION);
            }
            Some(IDENTIFIER) => {
                if detector.is_enum_detected() && declaration_parsing_mode.can_be_enum_used_as_soft_keyword() {
                    return Some(self.parse_class(true, false));
                }
            }
            _ => {}
        }

        None
    }
}
